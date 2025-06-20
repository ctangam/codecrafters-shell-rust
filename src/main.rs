#[allow(unused_imports)]
use std::io::{self, Write};
use std::process::exit;

fn main() {
    loop {
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
                    "echo" | "exit" => println!("{} is a shell builtin", arg),
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
