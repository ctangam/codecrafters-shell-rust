#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env, fs,
    path::Path,
    process::{exit, Command},
};

fn main() {
    loop {
        let paths = env::var("PATH").unwrap_or_default();
        let paths = if let "windows" = env::consts::OS {
            paths.split(';')
        } else {
            paths.split(':')
        };

        // Uncomment this block to pass the first stage
        print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        let mut parts = input.split_ascii_whitespace();
        let command = parts.next().unwrap();
        let args: Vec<&str> = parts.collect();
        match command {
            "pwd" => {
                println!("{}", env::current_dir().unwrap().display());
            }
            "echo" => {
                println!("{}", args.join(" "))
            }
            "type" => {
                let arg = args.first().unwrap();
                match *arg {
                    "echo" | "exit" | "type" | "pwd" => println!("{} is a shell builtin", arg),
                    cmd => {
                        if let Some(path) = search(paths, cmd) {
                            println!("{} is {}", cmd, path);
                        } else {
                            println!("{}: not found", arg);
                        }
                    }
                }
            }
            "exit" => {
                let code = args.first().map_or(0, |s| s.parse().unwrap());
                exit(code)
            }
            cmd => {
                if let Some(_path) = search(paths, cmd) {
                    let mut child = Command::new(cmd).args(args).spawn().unwrap();
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
