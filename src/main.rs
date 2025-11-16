use is_executable::IsExecutable;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

type MaybeRedirect = Option<String>;

enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(String),
    Unknown(String, Vec<String>),
    Pwd,
    Cd(String),
    Invalid,
}

impl Command {
    fn name(&self) -> String {
        match self {
            Command::Exit(_) => "exit".into(),
            Command::Echo(_) => "echo".into(),
            Command::Type(_) => "type".into(),
            Command::Unknown(name, _) => name.clone(),
            Command::Pwd => "pwd".into(),
            Command::Cd(_) => "cd".into(),
            Command::Invalid => unimplemented!(),
        }
    }
}

struct CommandWithContext {
    cmd: Command,
    stdout_redirect: MaybeRedirect,
}

const SHELL_BUILTIN_COMMANDS: [&'static str; 5] = ["echo", "type", "exit", "pwd", "cd"];

struct UnidentifiedCommand {
    name: String,
    args: Vec<String>,
    stdout_redirect: MaybeRedirect,
}

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
        if self.current() == '\\' {
            if self.state == CommandParseState::WordSection {
                self.next();
            } else if self.state == CommandParseState::DoubleQuoteSection {
                if self.has_n_more(1) {
                    if "\"$`\\\n".contains(self.peek()) {
                        self.next();
                    } else {
                        // Leave backslash.
                    }
                }
            } else if self.state == CommandParseState::SingleQuoteSection {
                // Do nothing.
            }
        }

        if !self.at_end() {
            self.buf.push(self.current());
        }
    }

    fn parse(mut self) -> Option<UnidentifiedCommand> {
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
            Some(ArgParser::build_unidentified_command(name, parts))
        }
    }

    fn build_unidentified_command(name: String, mut args: Vec<String>) -> UnidentifiedCommand {
        let mut stdout_redirect = None;

        if args.len() >= 2 {
            if args[args.len() - 2] == ">" || args[args.len() - 2] == "1>" {
                stdout_redirect = Some(args.pop().unwrap());
                args.pop().unwrap();
            }
        }

        UnidentifiedCommand {
            name,
            args,
            stdout_redirect,
        }
    }
}

fn parse_command(raw: &str) -> CommandWithContext {
    let raw_cmd = match ArgParser::new(raw).parse() {
        Some(v) => v,
        None => {
            return CommandWithContext {
                cmd: Command::Invalid,
                stdout_redirect: None,
            }
        }
    };

    let cmd = if raw_cmd.name == "exit" {
        let exit_code = if raw_cmd.args.len() == 1 {
            if let Ok(v) = i32::from_str_radix(&raw_cmd.args[0], 10) {
                v
            } else {
                return CommandWithContext {
                    cmd: Command::Invalid,
                    stdout_redirect: None,
                };
            }
        } else if raw_cmd.args.len() > 1 {
            return CommandWithContext {
                cmd: Command::Invalid,
                stdout_redirect: None,
            };
        } else {
            0
        };
        Command::Exit(exit_code)
    } else if raw_cmd.name == "echo" {
        Command::Echo(raw_cmd.args)
    } else if raw.starts_with("type") {
        if raw_cmd.args.len() != 1 {
            Command::Invalid
        } else {
            Command::Type(raw_cmd.args[0].clone())
        }
    } else if raw_cmd.name == "pwd" {
        Command::Pwd
    } else if raw_cmd.name == "cd" {
        if raw_cmd.args.len() != 1 {
            Command::Invalid
        } else {
            Command::Cd(raw_cmd.args[0].clone())
        }
    } else {
        Command::Unknown(raw_cmd.name, raw_cmd.args)
    };

    CommandWithContext {
        cmd,
        stdout_redirect: raw_cmd.stdout_redirect,
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

fn output(to_stdout: String, stdout_redirect: MaybeRedirect, original_cmd: &str) {
    if let Some(redirect_file) = stdout_redirect {
        if let Ok(mut f) = std::fs::File::create(&redirect_file) {
            f.write_all(to_stdout.as_bytes()).unwrap();
        } else {
            println!(
                "{}: {}: No such file or directory",
                original_cmd, redirect_file
            );
        }
    } else {
        println!("{}", to_stdout);
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

        let cmd_with_ctx = parse_command(buf.trim());
        let orig_cmd_name = cmd_with_ctx.cmd.name().clone();
        match cmd_with_ctx.cmd {
            Command::Exit(exit_code) => std::process::exit(exit_code),
            Command::Echo(parts) => {
                output(
                    format!("{}", parts.join(" ")),
                    cmd_with_ctx.stdout_redirect,
                    &orig_cmd_name,
                );
            }
            Command::Type(what) => {
                if SHELL_BUILTIN_COMMANDS.contains(&what.as_str()) {
                    output(
                        format!("{} is a shell builtin", what),
                        cmd_with_ctx.stdout_redirect,
                        &orig_cmd_name,
                    );
                } else {
                    match verify_executable(&what, &env_path) {
                        Some(path) => output(
                            format!("{} is {}", what, path),
                            cmd_with_ctx.stdout_redirect,
                            &orig_cmd_name,
                        ),
                        _ => output(
                            format!("{}: not found", what),
                            cmd_with_ctx.stdout_redirect,
                            &orig_cmd_name,
                        ),
                    }
                }
            }
            Command::Unknown(name, args) => {
                if let Ok(process_output) = std::process::Command::new(&name).args(&args).output() {
                    output(
                        String::from_utf8(process_output.stdout)
                            .unwrap()
                            .trim()
                            .into(),
                        cmd_with_ctx.stdout_redirect,
                        &orig_cmd_name,
                    );
                    eprintln!(
                        "{}",
                        String::from_utf8(process_output.stderr).unwrap().trim()
                    );
                } else {
                    output(
                        format!("{}: command not found", name),
                        cmd_with_ctx.stdout_redirect,
                        &orig_cmd_name,
                    );
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
                Err(_) => output(
                    format!("cd: {}: No such file or directory", path.to_string()),
                    cmd_with_ctx.stdout_redirect,
                    &orig_cmd_name,
                ),
            },
            Command::Invalid => output(
                format!("{}: command not found", buf.trim()),
                cmd_with_ctx.stdout_redirect,
                &orig_cmd_name,
            ),
        };

        io::stdout().flush().unwrap();
    }
}
