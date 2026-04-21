use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
    process::Stdio,
    rc::Rc,
};

use rustyline::{
    history::{DefaultHistory, History},
    Editor,
};

use crate::{command::Command, common::*, completion::BinaryAndFileCompleter, job::Jobs};

pub(crate) struct ShellContext {
    completions: Rc<RefCell<HashMap<String /* command */, String /* script path */>>>,
}

impl ShellContext {
    pub(crate) fn new(completions: Rc<RefCell<HashMap<String, String>>>) -> Self {
        Self { completions }
    }

    pub(crate) fn execute_command(
        &mut self,
        cmd_with_ctx: CommandWithContext,
        rl: &mut Editor<BinaryAndFileCompleter, DefaultHistory>,
        env_paths: &Vec<PathBuf>,
        original_input: &String,
        pipe_reader: Option<io::PipeReader>,
        pipe_writer: Option<io::PipeWriter>,
        last_history_save_index: &mut usize,
        history_file_name: &String,
        jobs: &mut Jobs,
    ) -> ExecutionResult {
        let orig_cmd_name = cmd_with_ctx.cmd.name().clone();

        if !verify_redirect_exist(&cmd_with_ctx.stdout_redirect, &orig_cmd_name) {
            return ExecutionResult::None;
        }
        if !verify_redirect_exist(&cmd_with_ctx.stderr_redirect, &orig_cmd_name) {
            return ExecutionResult::None;
        }

        match cmd_with_ctx.cmd {
            Command::Exit(exit_code) => {
                // let _ = rl.save_history(history_file_name);
                save_history(history_file_name, rl, false, last_history_save_index);
                std::process::exit(exit_code);
            }
            Command::Echo(parts) => {
                output(
                    format!("{}", parts.join(" ")),
                    cmd_with_ctx.stdout_redirect,
                    pipe_writer,
                );
                output_error(String::new(), cmd_with_ctx.stderr_redirect);
            }
            Command::Type(what) => {
                if SHELL_BUILTIN_COMMANDS.contains(&what.as_str()) {
                    output(
                        format!("{} is a shell builtin", what),
                        cmd_with_ctx.stdout_redirect,
                        pipe_writer,
                    );
                } else {
                    match verify_executable(&what, &env_paths) {
                        Some(path) => {
                            output(
                                format!("{} is {}", what, path),
                                cmd_with_ctx.stdout_redirect,
                                pipe_writer,
                            );
                            output_error(String::new(), cmd_with_ctx.stderr_redirect);
                        }
                        _ => output_error(
                            format!("{}: not found", what),
                            cmd_with_ctx.stderr_redirect,
                        ),
                    }
                }
            }
            Command::Unknown(name, args) => {
                let ref mut os_command = std::process::Command::new(&name);

                os_command.args(&args);

                if let Some(reader) = pipe_reader {
                    os_command.stdin(reader);
                }
                if let Some(writer) = pipe_writer {
                    os_command.stdout(writer);

                    return match os_command.spawn() {
                        Ok(child) => {
                            if cmd_with_ctx.is_job {
                                ExecutionResult::ChildForBackground {
                                    child,
                                    original: cmd_with_ctx.original,
                                }
                            } else {
                                ExecutionResult::Child { child }
                            }
                        }
                        Err(_) => {
                            output(
                                format!("{}: command not found", name),
                                cmd_with_ctx.stdout_redirect,
                                None,
                            );
                            ExecutionResult::None
                        }
                    };
                } else {
                    if cmd_with_ctx.stdout_redirect.is_some() {
                        os_command.stdout(Stdio::piped());
                    } else {
                        os_command.stdout(Stdio::inherit());
                    }
                    os_command.stderr(Stdio::piped());
                }

                if let Ok(child) = os_command.spawn() {
                    if cmd_with_ctx.is_job {
                        return ExecutionResult::ChildForBackground {
                            child,
                            original: cmd_with_ctx.original,
                        };
                    } else {
                        return ExecutionResult::ChildWithOutputHandling {
                            child,
                            stdout_redirect: cmd_with_ctx.stdout_redirect,
                            stderr_redirect: cmd_with_ctx.stderr_redirect,
                        };
                    }
                } else {
                    output(
                        format!("{}: command not found", name),
                        cmd_with_ctx.stdout_redirect,
                        None,
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
                pipe_writer,
            ),
            Command::Cd(path) => match std::env::set_current_dir(home_path_expand(path.clone())) {
                Ok(_) => {}
                Err(_) => output_error(
                    format!("cd: {}: No such file or directory", path.to_string()),
                    cmd_with_ctx.stderr_redirect,
                ),
            },
            Command::Jobs => {
                jobs.print_status_report(false);
            }
            Command::History(n) => {
                let mut history_str = String::new();
                let history_len = rl.history().len();
                let start_from = if n >= history_len { 0 } else { history_len - n };
                for (i, elem) in rl.history().iter().skip(start_from).enumerate() {
                    history_str.push_str(format!("\t{}  {}\n", i + 1 + start_from, elem).as_str());
                }
                output(
                    history_str.trim_end().into(),
                    cmd_with_ctx.stdout_redirect,
                    pipe_writer,
                );
                output_error(String::new(), cmd_with_ctx.stderr_redirect);
            }
            Command::HistoryAppend(path) => {
                append_to_history(path, rl);
                output(String::new(), cmd_with_ctx.stdout_redirect, pipe_writer);
                output_error(String::new(), cmd_with_ctx.stderr_redirect);
            }
            Command::HistorySave(path, should_append) => {
                save_history(&path, rl, should_append, last_history_save_index);
                output(String::new(), cmd_with_ctx.stdout_redirect, pipe_writer);
                output_error(String::new(), cmd_with_ctx.stderr_redirect);
            }
            Command::CompleteGet(cmd) => {
                if let Some(script_path) = self.completions.borrow().get(&cmd) {
                    output(
                        format!("complete -C '{}' {}", script_path, cmd),
                        cmd_with_ctx.stdout_redirect,
                        pipe_writer,
                    );
                } else {
                    output_error(
                        format!("complete: {}: no completion specification", cmd),
                        cmd_with_ctx.stderr_redirect,
                    );
                }
            }
            Command::CompleteSet {
                script_path,
                command,
            } => {
                self.completions.borrow_mut().insert(command, script_path);
            }
            Command::Empty => {}
            Command::Invalid => output_error(
                format!("{}: command not found", original_input.trim()),
                cmd_with_ctx.stdout_redirect,
            ),
        };

        io::stdout().flush().unwrap();

        ExecutionResult::None
    }
}
