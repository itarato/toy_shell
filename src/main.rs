#[allow(unused_imports)]
use std::io::{self, Write};

enum Command {
    Exit(i32),
    Unknown,
}

fn parse_command(raw: &str) -> Command {
    if raw.starts_with("exit") {
        let exit_code = if raw.len() > 4 {
            let parts = raw.split(' ').collect::<Vec<_>>();
            if parts.len() != 2 {
                return Command::Unknown;
            } else {
                if let Ok(v) = i32::from_str_radix(parts[1], 10) {
                    v
                } else {
                    return Command::Unknown;
                }
            }
        } else {
            0
        };
        Command::Exit(exit_code)
    } else {
        Command::Unknown
    }
}

fn main() {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut buf = String::new();
        io::stdin()
            .read_line(&mut buf)
            .expect("Failed reading STDIN");

        match parse_command(buf.trim()) {
            Command::Exit(exit_code) => std::process::exit(exit_code),
            Command::Unknown => println!("{}: command not found", buf.trim()),
        };

        io::stdout().flush().unwrap();
    }
}
