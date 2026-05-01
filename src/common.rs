use crate::arg_parser::*;
use crate::command::*;
use crate::completion::*;
use crate::redirect::*;
use is_executable::IsExecutable;
use rustyline::{
    history::{DefaultHistory, History},
    Editor,
};
use std::collections::HashMap;
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    fs,
    io::BufRead,
    path::{Path, PathBuf},
    process::Child,
};

pub(crate) const SHELL_BUILTIN_COMMANDS: [&'static str; 9] = [
    "echo", "type", "exit", "pwd", "cd", "history", "jobs", "complete", "declare",
];

pub(crate) fn parse_completion_args_from_line(line: &str) -> [String; 3] {
    let parts = line.split(" ").collect::<Vec<_>>();

    let command = parts[0].to_string();
    let current = parts.last().unwrap().to_string();
    let prev = if parts.len() >= 2 {
        parts[parts.len() - 2].to_string()
    } else {
        String::new()
    };

    [command, current, prev]
}

pub(crate) fn split_line_to_prefix_and_last_completion(line: &str) -> (&str, &str) {
    let last_space = line.len() - line.chars().rev().position(|c| c == ' ').unwrap();
    (&line[0..last_space - 1], &line[last_space..])
}

#[derive(Clone, Debug)]
pub(crate) struct CommandWithContext {
    pub(crate) original: String,
    pub(crate) cmd: Command,
    pub(crate) is_job: bool,
    pub(crate) stdout_redirect: MaybeRedirect,
    pub(crate) stderr_redirect: MaybeRedirect,
}

#[derive(Debug)]
pub(crate) struct PipedCommands(pub(crate) Vec<CommandWithContext>);

impl PipedCommands {
    fn new_invalid() -> Self {
        Self(vec![CommandWithContext {
            original: String::new(),
            cmd: Command::Invalid,
            is_job: false,
            stdout_redirect: None,
            stderr_redirect: None,
        }])
    }
}

pub(crate) fn parse_command(raw: &str, declared_vars: &HashMap<String, String>) -> PipedCommands {
    let mut raw_cmds = match ArgParser::new(raw).parse() {
        Some(v) => v.0,
        None => {
            return PipedCommands(vec![CommandWithContext {
                original: String::new(),
                cmd: Command::Invalid,
                is_job: false,
                stdout_redirect: None,
                stderr_redirect: None,
            }])
        }
    };

    let mut cmds_with_context = vec![];
    while !raw_cmds.is_empty() {
        let raw_cmd = raw_cmds.remove(0).replace_declared_vars(declared_vars);
        let cmd = if raw_cmd.name == "exit" {
            let exit_code = if raw_cmd.args.len() == 1 {
                if let Ok(v) = i32::from_str_radix(&raw_cmd.args[0], 10) {
                    v
                } else {
                    return PipedCommands::new_invalid();
                }
            } else if raw_cmd.args.len() > 1 {
                return PipedCommands::new_invalid();
            } else {
                0
            };
            Command::Exit(exit_code)
        } else if raw_cmd.name == "echo" {
            Command::Echo(raw_cmd.args)
        } else if raw_cmd.name == "type" {
            if raw_cmd.args.len() != 1 {
                Command::Invalid
            } else {
                Command::Type(raw_cmd.args[0].clone())
            }
        } else if raw_cmd.name == "pwd" {
            Command::Pwd
        } else if raw_cmd.name == "history" {
            if raw_cmd.args.is_empty() {
                Command::History(usize::MAX)
            } else if raw_cmd.args.len() == 1 {
                match usize::from_str_radix(&raw_cmd.args[0], 10) {
                    Ok(v) => Command::History(v),
                    Err(_) => return PipedCommands::new_invalid(),
                }
            } else if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-r" {
                Command::HistoryAppend(raw_cmd.args[1].clone().into())
            } else if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-w" {
                Command::HistorySave(raw_cmd.args[1].clone().into(), false)
            } else if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-a" {
                Command::HistorySave(raw_cmd.args[1].clone().into(), true)
            } else {
                return PipedCommands::new_invalid();
            }
        } else if raw_cmd.name == "cd" {
            if raw_cmd.args.len() != 1 {
                Command::Invalid
            } else {
                Command::Cd(raw_cmd.args[0].clone())
            }
        } else if raw_cmd.name == "jobs" {
            Command::Jobs
        } else if raw_cmd.name == "complete" {
            if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-r" {
                Command::CompleteRemove(raw_cmd.args[1].clone())
            } else if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-p" {
                Command::CompleteGet(raw_cmd.args[1].clone())
            } else if raw_cmd.args.len() == 3 && raw_cmd.args[0] == "-C" {
                let script_path = raw_cmd.args[1].clone();
                let command = raw_cmd.args[2].clone();
                Command::CompleteSet {
                    script_path,
                    command,
                }
            } else {
                Command::Invalid
            }
        } else if raw_cmd.name == "declare" {
            if raw_cmd.args.len() == 1 {
                let parts = raw_cmd.args[0].split('=').collect::<Vec<_>>();
                if parts.len() == 2 {
                    Command::Declare(parts[0].to_string(), parts[1].to_string())
                } else {
                    Command::Invalid
                }
            } else if raw_cmd.args.len() == 2 && raw_cmd.args[0] == "-p" {
                Command::DeclarePrint(raw_cmd.args[1].to_string())
            } else {
                Command::Invalid
            }
        } else if raw_cmd.name.is_empty() {
            Command::Empty
        } else {
            Command::Unknown(raw_cmd.name, raw_cmd.args)
        };

        cmds_with_context.push(CommandWithContext {
            original: raw_cmd.original,
            cmd,
            is_job: raw_cmd.is_job,
            stdout_redirect: raw_cmd.stdout_redirect,
            stderr_redirect: raw_cmd.stderr_redirect,
        });
    }

    PipedCommands(cmds_with_context)
}

pub(crate) fn verify_executable(name: &str, env_paths: &Vec<PathBuf>) -> Option<String> {
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

pub(crate) fn home_path_expand(path: String) -> String {
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

pub(crate) fn output(
    to_stdout: String,
    stdout_redirect: MaybeRedirect,
    maybe_pipe_writer: Option<io::PipeWriter>,
) {
    if let Some(mut pipe_writer) = maybe_pipe_writer {
        pipe_writer.write_all(to_stdout.as_bytes()).unwrap();
        pipe_writer.write_all(b"\n").unwrap();
    } else if let Some(redirect) = stdout_redirect {
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

pub(crate) fn output_error(to_stderr: String, stderr_redirect: MaybeRedirect) {
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

pub(crate) fn verify_redirect_exist(maybe_redirect: &MaybeRedirect, original_cmd: &str) -> bool {
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

pub(crate) fn preload_exec_names(env_paths: &Vec<PathBuf>) -> Vec<String> {
    let mut out = vec![];
    for path in env_paths {
        let Ok(entries) = std::fs::read_dir(path) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if !metadata.is_file() {
                continue;
            };

            let filename_os = entry.file_name();
            let Some(filename) = filename_os.to_str() else {
                continue;
            };

            out.push(filename.to_string());
        }
    }

    out
}

pub(crate) enum ExecutionResult {
    None,
    Child {
        child: Child,
    },
    ChildWithOutputHandling {
        child: Child,
        stdout_redirect: Option<Redirect>,
        stderr_redirect: Option<Redirect>,
    },
    ChildForBackground {
        child: Child,
        original: String,
    },
}

pub(crate) fn append_to_history(
    path: String,
    rl: &mut Editor<BinaryAndFileCompleter, DefaultHistory>,
) {
    let f = fs::File::open(path).unwrap();
    for line in io::BufReader::new(f).lines() {
        if let Ok(line) = line {
            if !line.is_empty() {
                rl.add_history_entry(line).unwrap();
            }
        }
    }
}

pub(crate) fn save_history(
    path: &String,
    rl: &mut Editor<BinaryAndFileCompleter, DefaultHistory>,
    should_append: bool,
    last_history_save_index: &mut usize,
) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(!should_append)
        .append(should_append)
        .open(path)
        .unwrap();

    let start_index = if should_append {
        *last_history_save_index
    } else {
        0
    };

    for line in rl.history().iter().skip(start_index) {
        f.write_all(line.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
    }

    if should_append {
        *last_history_save_index = rl.history().len();
    }
}

#[cfg(test)]
mod test {
    use crate::common::{
        parse_completion_args_from_line, split_line_to_prefix_and_last_completion,
    };

    #[test]
    fn test_parse_command_name() {
        assert_eq!(
            [
                "docker".to_string(),
                "123".to_string(),
                "something".to_string()
            ],
            parse_completion_args_from_line("docker something 123")
        );

        assert_eq!(
            [
                "docker".to_string(),
                "something".to_string(),
                "docker".to_string()
            ],
            parse_completion_args_from_line("docker something")
        );

        assert_eq!(
            [
                "docker".to_string(),
                "".to_string(),
                "something".to_string()
            ],
            parse_completion_args_from_line("docker something ")
        );
    }

    #[test]
    fn test_split_line_to_prefix_and_last_completion() {
        assert_eq!(
            ("docker something", "123"),
            split_line_to_prefix_and_last_completion("docker something 123"),
        );

        assert_eq!(
            ("docker something 123", ""),
            split_line_to_prefix_and_last_completion("docker something 123 "),
        );
    }
}
