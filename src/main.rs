#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env, fs,
    path::Path,
    process::{exit, Command, Stdio},
};

use anyhow::Result;

enum Symbol {
    Single(String),
    Double(String),
    Normal(String),
    Stdout(String),
    Whitespace,
}

fn main() -> Result<()> {
    loop {
        let paths = env::var("PATH").unwrap_or_default();
        let paths = if let "windows" = env::consts::OS {
            paths.split(';')
        } else {
            paths.split(':')
        };
        let home = env::var("HOME").unwrap_or("/".to_string());

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
        let (cmd, mut args, stdout, stderr) = parse(input);
        match &cmd[..] {
            "pwd" => {
                println!("{}", env::current_dir()?.display());
            }
            "cd" => {
                if let Some(target) = args.first() {
                    if target == "~" {
                        env::set_current_dir(home)?;
                    } else if fs::exists(&target)? {
                        env::set_current_dir(target)?;
                    } else {
                        println!("cd: {}: No such file or directory", target)
                    }
                }
            }
            "echo" => {
                let arg = args.concat();
                let stdout = if let Some(stdout) = stdout {
                    let fd = fs::File::create(stdout)?;
                    Stdio::from(fd)
                } else {
                    Stdio::inherit()
                };
                let stderr = if let Some(stderr) = stderr {
                    let fd = fs::File::create(stderr)?;
                    Stdio::from(fd)
                } else {
                    Stdio::inherit()
                };
                let mut child = Command::new(cmd)
                    .arg(arg)
                    .stdout(stdout)
                    .stderr(stderr)
                    .spawn()?;
                let _ = child.wait();
            }
            "type" => {
                if let Some(cmd) = args.first() {
                    match &cmd[..] {
                        "echo" | "exit" | "type" | "pwd" | "cd" => {
                            println!("{} is a shell builtin", cmd)
                        }
                        _ => {
                            if let Some(path) = search(paths, cmd) {
                                println!("{} is {}", cmd, path);
                            } else {
                                println!("{}: not found", cmd);
                            }
                        }
                    }
                }
            }
            "exit" => {
                let code = args.first().map_or(Ok(0), |s| s.parse())?;
                exit(code)
            }
            cmd => {
                if let Some(_path) = search(paths, cmd) {
                    args.retain(|s| !s.is_empty() && !s.trim().is_empty());
                    let stdout = if let Some(stdout) = stdout {
                        let fd = fs::File::create(stdout)?;
                        Stdio::from(fd)
                    } else {
                        Stdio::inherit()
                    };
                    let stderr = if let Some(stderr) = stderr {
                        let fd = fs::File::create(stderr)?;
                        Stdio::from(fd)
                    } else {
                        Stdio::inherit()
                    };
                    let mut child = Command::new(cmd)
                        .args(args)
                        .stdout(stdout)
                        .stderr(stderr)
                        .spawn()?;
                    let _ = child.wait();
                } else {
                    println!("{}: not found", cmd);
                }
            }
        }
    }
}

fn parse(input: &str) -> (String, Vec<String>, Option<String>, Option<String>) {
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
                i += 2;
                while i < input.len() {
                    s.push(input[i]);
                    i += 1;
                }
                stdout = Some(s);
                break;
            }
            '1' if input[i + 1] == '>' => {
                i += 1;
                continue;
            }
            '2' if input[i + 1] == '>' => {
                i += 3;
                while i < input.len() {
                    s.push(input[i]);
                    i += 1;
                }
                stderr = Some(s);
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

fn search<T>(paths: T, cmd: &str) -> Option<String>
where
    T: IntoIterator,
    T::Item: AsRef<Path> + std::fmt::Debug,
{
    for path in paths {
        if !fs::exists(&path).unwrap() {
            continue;
        }
        for entry in fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() == cmd {
                return Some(entry.path().to_string_lossy().into_owned());
            }
        }
    }
    None
}
