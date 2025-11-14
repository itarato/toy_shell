#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(String),
    Unknown,
}

const SHELL_BUILTIN_COMMANDS: [&'static str; 3] = ["echo", "type", "exit"];

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
    } else if raw.starts_with("type") {
        if raw.len() <= 5 {
            Command::Unknown
        } else {
            Command::Type(raw[5..].to_owned())
        }
    } else {
        Command::Unknown
    }
}

fn verify_executable(name: &str, env_paths: &Vec<&str>) -> Option<String> {
    for env_path in env_paths {
        let path = Path::new(&env_path).join(name);
        if let Ok(true) = std::fs::exists(&path) {
            return Some(path.to_str().unwrap().into());
        }
    }

    None
}

fn main() {
    let mut env_vars: HashMap<String, String> = HashMap::new();
    for (k, v) in std::env::vars() {
        env_vars.insert(k, v);
    }

    let env_path = env_vars
        .get("PATH")
        .map(|v| v.split(':').collect())
        .unwrap_or(vec![]);

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
            Command::Type(what) => {
                if SHELL_BUILTIN_COMMANDS.contains(&what.as_str()) {
                    println!("{} is a shell builtin", what);
                } else {
                    match verify_executable(&what, &env_path) {
                        Some(path) => println!("{} is {}", what, path),
                        _ => println!("{}: not found", what),
                    }
                }
            }
            Command::Unknown => println!("{}: command not found", buf.trim()),
        };

        io::stdout().flush().unwrap();
    }
}
