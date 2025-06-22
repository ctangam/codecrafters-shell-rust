#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env, fs,
    path::Path,
    process::{exit, Command},
};

use anyhow::Result;

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
        let (cmd, args) = input.split_once(' ').unwrap_or((input, ""));
        match cmd {
            "pwd" => {
                println!("{}", env::current_dir()?.display());
            }
            "cd" => {
                let target = args;
                if target == "~" {
                    env::set_current_dir(home)?;
                } else if fs::exists(target)? {
                    env::set_current_dir(target)?;
                } else {
                    println!("cd: {}: No such file or directory", target)
                }
            }
            "echo" => {
                let args = parse_args(args).concat();
                println!("{}", args)
            }
            "type" => match args {
                "echo" | "exit" | "type" | "pwd" | "cd" => {
                    println!("{} is a shell builtin", args)
                }
                cmd => {
                    if let Some(path) = search(paths, cmd) {
                        println!("{} is {}", cmd, path);
                    } else {
                        println!("{}: not found", args);
                    }
                }
            },
            "exit" => {
                let code = args.parse().unwrap_or_default();
                exit(code)
            }
            cmd => {
                if let Some(_path) = search(paths, cmd) {
                    let args = parse_args(args)
                        .into_iter()
                        .filter(|s| !s.is_empty() && !s.trim().is_empty())
                        .collect::<Vec<String>>();
                    let mut child = Command::new(cmd).args(args).spawn()?;
                    let _ = child.wait();
                } else {
                    println!("{}: not found", cmd);
                }
            }
        }
    }
}

fn parse_args(args: &str) -> Vec<String> {
    if args.contains('"') {
        args.split('"')
    } else {
        args.split('\'')
    }
    .enumerate()
    .flat_map(|(n, s)| {
        if n % 2 == 1 {
            vec![s.to_string()]
        } else {
            if !s.is_empty() && s.trim().is_empty() {
                vec![" ".to_string()]
            } else {
                let start = s.starts_with(' ');
                let end = s.ends_with(' ');
                let s = s.split_ascii_whitespace().collect::<Vec<&str>>().join(" ").replace("\\", "");
                let s = if start { format!(" {}", s) } else { s };
                let s = if end { format!("{} ", s) } else { s };
                s.split_inclusive(' ')
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            }
        }
    })
    .collect::<Vec<String>>()
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
