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
                let args = args
                    .split('\'')
                    .enumerate()
                    .map(|(n, s)| {
                        
                        if n % 2 == 1 {
                            s.to_string()
                        } else {
                            s.split_ascii_whitespace().collect::<Vec<&str>>().join(" ")
                        }
                    })
                    .filter(|s| !s.is_empty())
                    // .inspect(|s| println!("{}", s))
                    .collect::<Vec<String>>()
                    .join(" ");

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
                    let mut child = Command::new(cmd).arg(args).spawn()?;
                    let _ = child.wait();
                } else {
                    println!("{}: not found", cmd);
                }
            }
        }
    }
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
