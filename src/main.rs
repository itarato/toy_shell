use is_executable::IsExecutable;
use rustyline::{
    completion::{Candidate, Completer},
    history::DefaultHistory,
    line_buffer::LineBuffer,
    Changeset, Editor, Helper, Highlighter, Hinter, Validator,
};
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

mod arg_parser;
mod command;
mod redirect;

use arg_parser::*;
use command::*;
use redirect::*;

struct CommandWithContext {
    cmd: Command,
    stdout_redirect: MaybeRedirect,
    stderr_redirect: MaybeRedirect,
}

const SHELL_BUILTIN_COMMANDS: [&'static str; 5] = ["echo", "type", "exit", "pwd", "cd"];

fn parse_command(raw: &str) -> CommandWithContext {
    let raw_cmd = match ArgParser::new(raw).parse() {
        Some(v) => v,
        None => {
            return CommandWithContext {
                cmd: Command::Invalid,
                stdout_redirect: None,
                stderr_redirect: None,
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
                    stderr_redirect: None,
                };
            }
        } else if raw_cmd.args.len() > 1 {
            return CommandWithContext {
                cmd: Command::Invalid,
                stdout_redirect: None,
                stderr_redirect: None,
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
    } else if raw_cmd.name.is_empty() {
        Command::Empty
    } else {
        Command::Unknown(raw_cmd.name, raw_cmd.args)
    };

    CommandWithContext {
        cmd,
        stdout_redirect: raw_cmd.stdout_redirect,
        stderr_redirect: raw_cmd.stderr_redirect,
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

fn output(to_stdout: String, stdout_redirect: MaybeRedirect) {
    if let Some(redirect) = stdout_redirect {
        if let Ok(mut f) = redirect.file() {
            if !to_stdout.is_empty() {
                f.write_all(to_stdout.as_bytes()).unwrap();
                f.write_all(b"\n").unwrap();
            }
        } else {
            panic!("File was expected to exist");
        }
    } else {
        if !to_stdout.is_empty() {
            println!("{}", to_stdout);
        }
    }
}

fn output_error(to_stderr: String, stderr_redirect: MaybeRedirect) {
    if let Some(redirect) = stderr_redirect {
        if let Ok(mut f) = redirect.file() {
            if !to_stderr.is_empty() {
                f.write_all(to_stderr.as_bytes()).unwrap();
                f.write_all(b"\n").unwrap();
            }
        } else {
            panic!("File was expected to exist");
        }
    } else {
        if !to_stderr.is_empty() {
            eprintln!("{}", to_stderr);
        }
    }
}

fn verify_redirect_exist(maybe_redirect: &MaybeRedirect, original_cmd: &str) -> bool {
    if let Some(redirect) = maybe_redirect {
        if let Ok(_) = redirect.file() {
            true
        } else {
            eprintln!(
                "{}: {}: No such file or directory",
                original_cmd, redirect.filename
            );
            false
        }
    } else {
        true
    }
}

struct CustomRLCandidate {
    word: String,
}

impl Candidate for CustomRLCandidate {
    fn display(&self) -> &str {
        &self.word
    }

    fn replacement(&self) -> &str {
        &self.word
    }
}

#[derive(Helper, Validator, Highlighter, Hinter)]
struct CustomRLCompleter {}

impl Completer for CustomRLCompleter {
    type Candidate = CustomRLCandidate;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if line.is_empty() {
            return Ok((pos, vec![]));
        }

        for cmd in SHELL_BUILTIN_COMMANDS {
            if cmd.starts_with(line) {
                return Ok((0, vec![CustomRLCandidate { word: cmd.into() }]));
            }
        }

        Ok((pos, vec![]))
    }

    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str, cl: &mut Changeset) {
        let end = line.pos();
        let mut elected_with_space = elected.to_owned();
        elected_with_space.push(' ');
        line.replace(start..end, &elected_with_space, cl);
    }
}

impl CustomRLCompleter {
    fn new() -> Self {
        Self {}
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

    let rl_completer = CustomRLCompleter::new();
    let mut rl: Editor<CustomRLCompleter, DefaultHistory> = Editor::new().unwrap();

    rl.set_helper(Some(rl_completer));
    let _ = rl.load_history("history.txt");

    loop {
        let buf = match rl.readline("$ ") {
            Ok(s) => {
                rl.add_history_entry(&s).unwrap();
                s
            }
            Err(err) => {
                dbg!(err);
                continue;
            }
        };

        let cmd_with_ctx = parse_command(buf.trim());
        let orig_cmd_name = cmd_with_ctx.cmd.name().clone();

        if !verify_redirect_exist(&cmd_with_ctx.stdout_redirect, &orig_cmd_name) {
            continue;
        }
        if !verify_redirect_exist(&cmd_with_ctx.stderr_redirect, &orig_cmd_name) {
            continue;
        }

        match cmd_with_ctx.cmd {
            Command::Exit(exit_code) => {
                let _ = rl.save_history("history.txt");
                std::process::exit(exit_code);
            }
            Command::Echo(parts) => {
                output(format!("{}", parts.join(" ")), cmd_with_ctx.stdout_redirect);
                output_error(String::new(), cmd_with_ctx.stderr_redirect);
            }
            Command::Type(what) => {
                if SHELL_BUILTIN_COMMANDS.contains(&what.as_str()) {
                    output(
                        format!("{} is a shell builtin", what),
                        cmd_with_ctx.stdout_redirect,
                    );
                } else {
                    match verify_executable(&what, &env_path) {
                        Some(path) => output(
                            format!("{} is {}", what, path),
                            cmd_with_ctx.stdout_redirect,
                        ),
                        _ => output(format!("{}: not found", what), cmd_with_ctx.stdout_redirect),
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
                    );

                    output_error(
                        String::from_utf8(process_output.stderr)
                            .unwrap()
                            .trim()
                            .into(),
                        cmd_with_ctx.stderr_redirect,
                    );
                } else {
                    output(
                        format!("{}: command not found", name),
                        cmd_with_ctx.stdout_redirect,
                    );
                }
            }
            Command::Pwd => output(
                format!(
                    "{}",
                    std::env::current_dir()
                        .expect("Cannot retrieve current work dir")
                        .to_str()
                        .expect("Cannot stringify path")
                ),
                cmd_with_ctx.stdout_redirect,
            ),
            Command::Cd(path) => match std::env::set_current_dir(home_path_expand(path.clone())) {
                Ok(_) => {}
                Err(_) => output(
                    format!("cd: {}: No such file or directory", path.to_string()),
                    cmd_with_ctx.stdout_redirect,
                ),
            },
            Command::Empty => {}
            Command::Invalid => output(
                format!("{}: command not found", buf.trim()),
                cmd_with_ctx.stdout_redirect,
            ),
        };

        io::stdout().flush().unwrap();
    }
}
