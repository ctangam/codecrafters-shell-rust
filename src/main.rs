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

        match &input[..] {
            "exit 0" => {
                exit(0)
            }
            input => {
                println!("{}: command not found", input.trim())
            }
        }
    }
}
