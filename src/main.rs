use is_executable::IsExecutable;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(String),
    Unknown(String, Vec<String>),
    Pwd,
    Cd(String),
    Invalid,
}

const SHELL_BUILTIN_COMMANDS: [&'static str; 5] = ["echo", "type", "exit", "pwd", "cd"];

#[derive(PartialEq, Eq)]
enum CommandParseState {
    DropSection,
    WordSection,
    SingleQuoteSection,
    DoubleQuoteSection,
}

struct ArgParser {
    chars: Vec<char>,
    i: usize,
    buf: String,
    state: CommandParseState,
}

impl ArgParser {
    fn new(raw: &str) -> Self {
        Self {
            chars: raw.chars().collect(),
            i: 0,
            buf: String::new(),
            state: CommandParseState::DropSection,
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.chars.len()
    }

    fn current(&self) -> char {
        self.chars[self.i]
    }

    fn has_n_more(&self, n: usize) -> bool {
        self.chars.len() > self.i + n
    }

    fn peek(&self) -> char {
        self.peekn(1)
    }

    fn peekn(&self, n: usize) -> char {
        self.chars[self.i + n]
    }

    fn next(&mut self) {
        self.i += 1;
    }

    fn push(&mut self) {
        if self.current() == '\\' && self.state == CommandParseState::WordSection {
            self.next();
        }

        if !self.at_end() {
            self.buf.push(self.current());
        }
    }

    fn parse(mut self) -> Option<(String, Vec<String>)> {
        let mut parts: Vec<String> = vec![];

        while !self.at_end() {
            let c = self.current();

            match self.state {
                CommandParseState::DropSection => {
                    if c.is_whitespace() {
                        // Noop.
                    } else if c == '\'' {
                        self.state = CommandParseState::SingleQuoteSection;
                    } else if c == '"' {
                        self.state = CommandParseState::DoubleQuoteSection;
                    } else {
                        self.state = CommandParseState::WordSection;
                        self.push();
                    }
                }
                CommandParseState::SingleQuoteSection => {
                    if c == '\'' {
                        if self.has_n_more(1) && !self.peek().is_whitespace() {
                            if self.peek() == '\'' {
                                self.next();
                            } else if self.peek() == '"' {
                                self.next();
                                self.state = CommandParseState::DoubleQuoteSection;
                            } else {
                                self.state = CommandParseState::WordSection;
                            }
                        } else {
                            self.state = CommandParseState::DropSection;
                            parts.push(self.buf.clone());
                            self.buf.clear();
                        }
                    } else {
                        self.push();
                    }
                }
                CommandParseState::DoubleQuoteSection => {
                    if c == '"' {
                        if self.has_n_more(1) && !self.peek().is_whitespace() {
                            if self.peek() == '\'' {
                                self.next();
                                self.state = CommandParseState::SingleQuoteSection;
                            } else if self.peek() == '"' {
                                self.next();
                            } else {
                                self.state = CommandParseState::WordSection;
                            }
                        } else {
                            self.state = CommandParseState::DropSection;
                            parts.push(self.buf.clone());
                            self.buf.clear();
                        }
                    } else {
                        self.push();
                    }
                }
                CommandParseState::WordSection => {
                    if c.is_whitespace() {
                        self.state = CommandParseState::DropSection;
                        parts.push(self.buf.clone());
                        self.buf.clear();
                    } else if c == '\'' {
                        self.state = CommandParseState::SingleQuoteSection;
                    } else if c == '"' {
                        self.state = CommandParseState::DoubleQuoteSection;
                    } else {
                        self.push();
                    }
                }
            };

            self.next();
        }

        if let CommandParseState::WordSection = self.state {
            parts.push(self.buf.clone());
        }

        if parts.len() < 1 {
            None
        } else {
            let name = parts.remove(0);
            Some((name, parts))
        }
    }
}

fn parse_command(raw: &str) -> Command {
    let (name, args) = match ArgParser::new(raw).parse() {
        Some(name_args_pair) => name_args_pair,
        None => return Command::Invalid,
    };

    if name == "exit" {
        let exit_code = if args.len() == 1 {
            if let Ok(v) = i32::from_str_radix(&args[0], 10) {
                v
            } else {
                return Command::Invalid;
            }
        } else if args.len() > 1 {
            return Command::Invalid;
        } else {
            0
        };
        Command::Exit(exit_code)
    } else if name == "echo" {
        Command::Echo(args)
    } else if raw.starts_with("type") {
        if args.len() != 1 {
            Command::Invalid
        } else {
            Command::Type(args[0].clone())
        }
    } else if name == "pwd" {
        Command::Pwd
    } else if name == "cd" {
        if args.len() != 1 {
            Command::Invalid
        } else {
            Command::Cd(args[0].clone())
        }
    } else {
        Command::Unknown(name, args)
    }
}

fn verify_executable(name: &str, env_paths: &Vec<PathBuf>) -> Option<String> {
    for env_path in env_paths {
        let path = Path::new(&env_path).join(name);
        if let Ok(true) = std::fs::exists(&path) {
            if path.is_executable() {
                return Some(path.to_str().unwrap().into());
            }
        }
    }

    None
}

fn home_path_expand(path: String) -> String {
    if path == "~" {
        std::env::home_dir()
            .expect("Failed getting home dir")
            .to_str()
            .expect("Failed to convert to string")
            .into()
    } else if path.starts_with("~/") {
        std::env::home_dir()
            .expect("Failed getting home dir")
            .join(&path[2..])
            .to_str()
            .expect("Failed to convert to string")
            .into()
    } else {
        path
    }
}

fn main() {
    let mut env_vars: HashMap<String, String> = HashMap::new();
    for (k, v) in std::env::vars() {
        env_vars.insert(k, v);
    }

    let env_path = env_vars
        .get("PATH")
        .map(|v| std::env::split_paths(v).collect())
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
            Command::Unknown(name, args) => {
                if let Ok(mut child) = std::process::Command::new(&name).args(&args).spawn() {
                    child.wait().expect("Failed waiting for children");
                } else {
                    println!("{}: command not found", name);
                }
            }
            Command::Pwd => println!(
                "{}",
                std::env::current_dir()
                    .expect("Cannot retrieve current work dir")
                    .to_str()
                    .expect("Cannot stringify path")
            ),
            Command::Cd(path) => match std::env::set_current_dir(home_path_expand(path.clone())) {
                Ok(_) => {}
                Err(_) => println!("cd: {}: No such file or directory", path.to_string()),
            },
            Command::Invalid => println!("{}: command not found", buf.trim()),
        };

        io::stdout().flush().unwrap();
    }
}
