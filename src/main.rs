#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::exit,
};

fn main() {
    loop {
        let paths = env::var("PATH").unwrap_or_default();
        let paths = paths.split(':');

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
            "echo" => {
                println!("{}", args.join(" "))
            }
            "type" => {
                let arg = args.first().unwrap();
                match *arg {
                    "echo" | "exit" | "type" => println!("{} is a shell builtin", arg),
                    cmd if search(paths, cmd).is_some() => {
                        println!("{} is an external command", cmd);
                    }
                    _ => println!("{}: not found", arg),
                }
            }
            "exit" => {
                let code = args.first().map_or(0, |s| s.parse().unwrap());
                exit(code)
            }
            input => {
                println!("{}: command not found", input)
            }
        }
    }
}

fn search<T>(paths: T, cmd: &str) -> Option<PathBuf>
where
    T: Iterator,
    T::Item: AsRef<Path>,
{
    for path in paths {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            print!("{:?}", entry.file_name());
            if entry.file_name() == cmd {
                return Some(entry.path());
            }
        }
    }
    None
}
