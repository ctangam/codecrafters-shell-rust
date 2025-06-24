#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env,
    fs::{self, read_to_string, OpenOptions},
    io::Read,
    path::PathBuf,
    process::{exit, Command, Stdio},
};

use anyhow::Result;

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

fn main() -> Result<()> {
    let paths = env::var("PATH").unwrap_or_default();
    let paths: Vec<&str> = if let "windows" = env::consts::OS {
        paths.split(';').collect()
    } else {
        paths.split(':').collect()
    };

    let home = env::var("HOME").unwrap_or("/".to_string());
    let mut history = Vec::new();
    let mut history_path = None;
    loop {
        // Uncomment this block to pass the first stage
        print!("$ ");
        io::stdout().flush()?;

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        history.push(input.to_string());
        let mut commands = input.split(" | ").peekable();
        let mut prev_stdout = None;
        let mut children = Vec::new();
        while let Some(input) = commands.next() {
            let (cmd, mut args, stdout, stderr) = parse(input);
            match &cmd[..] {
                "history" => {
                    if args.first() == Some(&"-r".to_string()) {
                        if let Some(path) = args.last() {
                            load_history(path, &mut history)?;
                            history_path = Some(path.clone());
                        }

                        continue;
                    }
                    let n = args.first().map_or(Ok(history.len()), |s| s.parse())?;
                    history
                        .iter()
                        .enumerate()
                        .skip(history.len() - n)
                        .for_each(|(i, s)| println!("    {}  {}", i + 1, s));
                }
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
                        save_history(path, &history)?;
                    }
                    let code = args.first().map_or(Ok(0), |s| s.parse())?;
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

fn load_history(path: &str, history: &mut Vec<String>) -> Result<()> {
    for line in read_to_string(path)?.lines() {
        history.push(line.to_string())
    }
    Ok(())
}

fn save_history(path: &str, history: &Vec<String>) -> Result<()> {
    let mut fd = fs::File::create(path)?;
    for line in history {
        writeln!(&mut fd, "{}", line)?;
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
