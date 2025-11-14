#[allow(unused_imports)]
use std::io::{self, Write};

enum Command {
    Exit(i32),
    Echo(Vec<String>),
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
    } else if raw.starts_with("echo") {
        let parts = raw
            .split(' ')
            .skip(1)
            .filter(|s| s.len() > 0)
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        Command::Echo(parts)
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
            Command::Echo(parts) => println!("{}", parts.join(" ")),
            Command::Unknown => println!("{}: command not found", buf.trim()),
        };

        io::stdout().flush().unwrap();
    }
}
