mod arg_parser;
mod command;
mod common;
mod completion;
mod job;
mod redirect;
mod shell_context;

use completion::*;
use rustyline::{history::DefaultHistory, Config, Editor};
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{common::*, job::Jobs, shell_context::ShellContext};

fn main() {
    let mut env_vars: HashMap<String, String> = HashMap::new();
    for (k, v) in std::env::vars() {
        env_vars.insert(k, v);
    }

    let env_paths = env_vars
        .get("PATH")
        .map(|v| std::env::split_paths(v).collect())
        .unwrap_or(vec![]);

    let completions = Rc::new(RefCell::new(HashMap::new()));
    let rl_completer =
        BinaryAndFileCompleter::new(preload_exec_names(&env_paths), completions.clone());
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut rl: Editor<BinaryAndFileCompleter, DefaultHistory> =
        Editor::with_config(config).unwrap();

    let history_file_name = env_vars
        .get("HISTFILE")
        .cloned()
        .unwrap_or("history.txt".to_string());

    rl.set_helper(Some(rl_completer));
    let _ = rl.load_history(&history_file_name);
    let mut last_history_save_index = 0usize;

    let mut jobs = Jobs::new();
    let mut shell_ctx = ShellContext::new(completions);
    let mut declared_vars: HashMap<String, String> = HashMap::new();

    loop {
        jobs.print_status_report(true);

        let buf = match rl.readline("$ ") {
            Ok(s) => {
                rl.add_history_entry(&s).unwrap();
                s
            }
            Err(_err) => {
                continue;
            }
        };

        let mut exec_results = vec![];
        let mut piped_cmds = parse_command(buf.trim(), &declared_vars).0;

        let mut pipe_reader: Option<io::PipeReader> = None;
        let mut pipe_writer: Option<io::PipeWriter> = None;

        while !piped_cmds.is_empty() {
            let cmd_with_ctx = piped_cmds.remove(0);
            let (pr, pw) = io::pipe().expect("Failed_making pipe");

            if !piped_cmds.is_empty() {
                pipe_writer = Some(pw);
            }

            let result = shell_ctx.execute_command(
                cmd_with_ctx,
                &mut rl,
                &env_paths,
                &buf,
                pipe_reader.take(),
                pipe_writer.take(),
                &mut last_history_save_index,
                &history_file_name,
                &mut jobs,
                &mut declared_vars,
            );
            exec_results.push(result);

            pipe_reader = Some(pr);
        }

        for exec_result in exec_results {
            match exec_result {
                ExecutionResult::Child { mut child } => {
                    child.wait().unwrap();
                }
                ExecutionResult::ChildWithOutputHandling {
                    child,
                    stdout_redirect,
                    stderr_redirect,
                } => {
                    let child_output = child.wait_with_output().unwrap();

                    output(
                        String::from_utf8(child_output.stdout)
                            .unwrap()
                            .trim_end()
                            .into(),
                        stdout_redirect,
                        None,
                    );

                    output_error(
                        String::from_utf8(child_output.stderr)
                            .unwrap()
                            .trim_end()
                            .into(),
                        stderr_redirect,
                    );
                }
                ExecutionResult::ChildForBackground { child, original } => {
                    let pid = child.id();
                    jobs.push(child, original);
                    println!("[{}] {}", jobs.len(), pid);
                }
                ExecutionResult::None => {}
            };
        }
    }
}
