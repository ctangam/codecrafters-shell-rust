#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    borrow::Cow, collections::HashSet, env, fmt::Display, fs::{self, read_to_string, OpenOptions}, io::Read, path::{self, PathBuf}, process::{exit, Command, Stdio}
};
use std::borrow::Cow::{Borrowed, Owned};
use anyhow::Result;
use rustyline::{hint::HistoryHinter, history::DefaultHistory, Context, Hinter};
use rustyline::{
    hint::{Hint, Hinter},
    Cmd, ConditionalEventHandler, DefaultEditor, Editor, Event, EventContext, EventHandler,
    KeyEvent, RepeatCount,
};
use rustyline::{Completer, Helper, Highlighter, Validator};
use rustyline::highlight::Highlighter;

enum Symbol {
    Single(String),
    Double(String),
    Normal(String),
    Whitespace,
}

enum Mode {
    Create(String),
    Append(String),
}

enum State {
    Saved(String),
    New(String),
}

impl Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New(s) => write!(f, "{s}"),
            Self::Saved(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Completer, Helper, Hinter, Validator)]
struct MyHelper(#[rustyline(Hinter)] HistoryHinter);

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Owned(format!("\x1b[1;32m{prompt}\x1b[m"))
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[1m{hint}\x1b[m"))
    }
}

#[derive(Clone)]
struct CompleteHintHandler;
impl ConditionalEventHandler for CompleteHintHandler {
    fn handle(&self, evt: &Event, _: RepeatCount, _: bool, ctx: &EventContext) -> Option<Cmd> {
        if !ctx.has_hint() {
            return None; // default
        }
        if let Some(k) = evt.get(0) {
            #[allow(clippy::if_same_then_else)]
            if *k == KeyEvent::from('\t') {
                Some(Cmd::CompleteHint)
            } else if *k == KeyEvent::alt('f') && ctx.line().len() == ctx.pos() {
                let text = ctx.hint_text()?;
                let mut start = 0;
                if let Some(first) = text.chars().next() {
                    if !first.is_alphanumeric() {
                        start = text.find(|c: char| c.is_alphanumeric()).unwrap_or_default();
                    }
                }

                let text = text
                    .chars()
                    .enumerate()
                    .take_while(|(i, c)| *i <= start || c.is_alphanumeric())
                    .map(|(_, c)| c)
                    .collect::<String>();

                Some(Cmd::Insert(1, text))
            } else {
                None
            }
        } else {
            unreachable!()
        }
    }
}


fn main() -> Result<()> {
    let mut rl = Editor::<MyHelper, DefaultHistory>::new()?;
    rl.set_helper(Some(MyHelper(HistoryHinter::new())));

    let ceh = Box::new(CompleteHintHandler);
    rl.bind_sequence(KeyEvent::from('\t'), EventHandler::Conditional(ceh.clone()));
    rl.bind_sequence(KeyEvent::alt('f'), EventHandler::Conditional(ceh));
    rl.bind_sequence(
        Event::KeySeq(vec![KeyEvent::ctrl('X'), KeyEvent::ctrl('E')]),
        EventHandler::Simple(Cmd::Suspend), // TODO external editor
    );

    let paths = env::var("PATH").unwrap_or_default();
    let paths: Vec<&str> = if let "windows" = env::consts::OS {
        paths.split(';').collect()
    } else {
        paths.split(':').collect()
    };

    let home = env::var("HOME").unwrap_or_default();

    let mut history = Vec::new();
    let history_path = env::var("HISTFILE").ok();
    if let Some(path) = &history_path {
        load_history(path, &mut history)?;
    }

    loop {
        let readline = rl.readline("$ ")?;
        rl.add_history_entry(readline.as_str())?;
        let input = readline.trim();
        if input.is_empty() {
            continue;
        }
        history.push(State::New(input.to_string()));
        let mut commands = input.split(" | ").peekable();
        let mut prev_stdout = None;
        let mut children = Vec::new();
        while let Some(input) = commands.next() {
            let (cmd, mut args, stdout, stderr) = parse(input);
            match &cmd[..] {
                "history" => match args.first() {
                    Some(val) if val == "-r" => {
                        if let Some(path) = args.last() {
                            load_history(path, &mut history)?;
                        }
                    }
                    Some(val) if val == "-w" => {
                        if let Some(path) = args.last() {
                            save_history(path, &history)?;
                        }
                    }
                    Some(val) if val == "-a" => {
                        if let Some(path) = args.last() {
                            append_history(path, &mut history)?;
                        }
                    }
                    val => {
                        let n = val.map_or(Ok(history.len()), |s| s.parse())?;
                        history
                            .iter()
                            .enumerate()
                            .skip(history.len() - n)
                            .for_each(|(i, s)| println!("    {}  {}", i + 1, s));
                    }
                },
                "pwd" => {
                    println!("{}", env::current_dir()?.display());
                }
                "cd" => {
                    if let Some(target) = args.first() {
                        if target == "~" {
                            env::set_current_dir(&home)?;
                        } else if fs::exists(target)? {
                            env::set_current_dir(target)?;
                        } else {
                            println!("cd: {}: No such file or directory", target)
                        }
                    }
                }
                "echo" => {
                    let arg = args.concat();
                    let stdin = match prev_stdout.take() {
                        Some(output) => Stdio::from(output),
                        None => Stdio::inherit(),
                    };
                    let stdout = if commands.peek().is_some() {
                        Stdio::piped()
                    } else {
                        match stdout {
                            Some(Mode::Append(stdout)) => {
                                let fd =
                                    OpenOptions::new().create(true).append(true).open(stdout)?;
                                Stdio::from(fd)
                            }
                            Some(Mode::Create(stdout)) => {
                                let fd = fs::File::create(stdout)?;
                                Stdio::from(fd)
                            }
                            None => Stdio::inherit(),
                        }
                    };
                    let stderr = match stderr {
                        Some(Mode::Append(stderr)) => {
                            let fd = OpenOptions::new().create(true).append(true).open(stderr)?;
                            Stdio::from(fd)
                        }
                        Some(Mode::Create(stderr)) => {
                            let fd = fs::File::create(stderr)?;
                            Stdio::from(fd)
                        }
                        None => Stdio::inherit(),
                    };
                    let mut child = Command::new(cmd)
                        .arg(arg)
                        .stdin(stdin)
                        .stdout(stdout)
                        .stderr(stderr)
                        .spawn()?;
                    prev_stdout = child.stdout.take();
                    children.push(child);
                }
                "type" => {
                    if let Some(cmd) = args.first() {
                        match &cmd[..] {
                            "echo" | "exit" | "type" | "pwd" | "cd" | "history" => {
                                println!("{} is a shell builtin", cmd)
                            }
                            _ => {
                                if let Ok(Some(path)) = search(&paths[..], cmd) {
                                    println!("{} is {}", cmd, path.display());
                                } else {
                                    println!("{}: not found", cmd);
                                }
                            }
                        }
                    }
                }
                "exit" => {
                    if let Some(path) = &history_path {
                        append_history(path, &mut history)?;
                    }
                    let code = args
                        .first()
                        .map_or(0, |s| s.parse().ok().unwrap_or_default());
                    exit(code)
                }
                cmd => {
                    if let Ok(Some(_path)) = search(&paths[..], cmd) {
                        args.retain(|s| !s.is_empty() && !s.trim().is_empty());
                        let stdin = match prev_stdout.take() {
                            Some(output) => Stdio::from(output),
                            None => Stdio::inherit(),
                        };
                        let stdout = if commands.peek().is_some() {
                            Stdio::piped()
                        } else {
                            match stdout {
                                Some(Mode::Append(stdout)) => {
                                    let fd = OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(stdout)?;
                                    Stdio::from(fd)
                                }
                                Some(Mode::Create(stdout)) => {
                                    let fd = fs::File::create(stdout)?;
                                    Stdio::from(fd)
                                }
                                None => Stdio::inherit(),
                            }
                        };
                        let stderr = match stderr {
                            Some(Mode::Append(stderr)) => {
                                let fd =
                                    OpenOptions::new().create(true).append(true).open(stderr)?;
                                Stdio::from(fd)
                            }
                            Some(Mode::Create(stderr)) => {
                                let fd = fs::File::create(stderr)?;
                                Stdio::from(fd)
                            }
                            None => Stdio::inherit(),
                        };
                        let mut child = Command::new(cmd)
                            .args(args)
                            .stdin(stdin)
                            .stdout(stdout)
                            .stderr(stderr)
                            .spawn()?;
                        prev_stdout = child.stdout.take();
                        children.push(child);
                    } else {
                        println!("{}: not found", cmd);
                    }
                }
            }
        }
        for mut child in children {
            child.wait()?;
        }
    }
}

fn load_history(path: &str, history: &mut Vec<State>) -> Result<()> {
    for line in read_to_string(path)?.lines() {
        history.push(State::Saved(line.to_string()))
    }
    Ok(())
}

fn save_history(path: &str, history: &Vec<State>) -> Result<()> {
    let mut fd = fs::File::create(path)?;
    for line in history {
        match line {
            State::Saved(s) => writeln!(&mut fd, "{}", s)?,
            State::New(s) => writeln!(&mut fd, "{}", s)?,
        }
    }
    Ok(())
}

fn append_history(path: &str, history: &mut Vec<State>) -> Result<()> {
    let mut fd = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(path)?;
    for line in history {
        match line {
            State::Saved(_) => continue,
            State::New(s) => {
                writeln!(&mut fd, "{}", s)?;
                *line = State::Saved(s.to_string())
            }
        }
    }
    Ok(())
}

fn parse(input: &str) -> (String, Vec<String>, Option<Mode>, Option<Mode>) {
    let input = input.chars().collect::<Vec<char>>();
    let mut i = 0;
    let mut args = Vec::new();
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if i >= input.len() {
            break;
        }
        let mut s = String::new();
        match input[i] {
            '>' => {
                if input[i + 1] == '>' {
                    i += 3;
                    while i < input.len() {
                        s.push(input[i]);
                        i += 1;
                    }
                    stdout = Some(Mode::Append(s))
                } else {
                    i += 2;
                    while i < input.len() {
                        s.push(input[i]);
                        i += 1;
                    }
                    stdout = Some(Mode::Create(s));
                }

                break;
            }
            '1' if input[i + 1] == '>' => {
                i += 1;
                continue;
            }
            '2' if input.get(i + 1) == Some(&'>') => {
                if input[i + 2] == '>' {
                    i += 4;
                    while i < input.len() {
                        s.push(input[i]);
                        i += 1;
                    }
                    stderr = Some(Mode::Append(s));
                } else {
                    i += 3;
                    while i < input.len() {
                        s.push(input[i]);
                        i += 1;
                    }
                    stderr = Some(Mode::Create(s));
                }
                break;
            }
            '"' => {
                i += 1;
                while i < input.len() && input[i] != '"' {
                    if input[i] == '\'' {
                        // Handle single quotes inside double quotes
                        s.push(input[i]);
                        i += 1;
                        while i < input.len() && input[i] != '\'' && input[i] != '"' {
                            s.push(input[i]);
                            i += 1;
                        }
                        if input[i] == '\'' {
                            s.push(input[i]);
                            i += 1; // Skip the closing quote
                        }
                    } else if input[i] == '\\' && (input[i + 1] == '"' || input[i + 1] == '\\') {
                        i += 1; // Skip the escape character
                        s.push(input[i]);
                        i += 1;
                    } else {
                        s.push(input[i]);
                        i += 1;
                    }
                }
                i += 1; // Skip the closing quote
            }
            '\'' => {
                i += 1;
                while i < input.len() && input[i] != '\'' {
                    s.push(input[i]);
                    i += 1;
                }
                i += 1; // Skip the closing quote
            }
            ' ' => {
                i += 1;
                if input[i] == '>' || input.get(i + 1) == Some(&'>') {
                    continue;
                }
                s.push(' ');
                while i < input.len() && input[i] == ' ' {
                    i += 1;
                }
            }
            _ => {
                while i < input.len() {
                    if input[i].is_whitespace() {
                        break;
                    }
                    if input[i] == '"' || input[i] == '\'' {
                        break;
                    }
                    if input[i] == '\\' {
                        i += 1; // Skip the escape character
                    }
                    s.push(input[i]);
                    i += 1;
                }
            }
        }
        args.push(s);
    }

    let cmd = args.remove(0);
    if !args.is_empty() {
        args.remove(0);
    }
    (cmd, args, stdout, stderr)
}

fn search(paths: &[&str], cmd: &str) -> Result<Option<PathBuf>> {
    for path in paths {
        if !fs::exists(path).unwrap() {
            continue;
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_name() == cmd {
                return Ok(Some(entry.path()));
            }
        }
    }
    Ok(None)
}
