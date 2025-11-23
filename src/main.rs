use is_executable::IsExecutable;
use rustyline::{
    completion::{Candidate, Completer},
    history::{DefaultHistory, History},
    line_buffer::LineBuffer,
    Changeset, Editor, Helper, Highlighter, Hinter, Validator,
};
#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    cell::Cell,
    collections::{BTreeSet, HashMap},
    fs,
    io::BufRead,
    path::{Path, PathBuf},
    process::{Child, Stdio},
};

mod arg_parser;
mod command;
mod redirect;

use arg_parser::*;
use command::*;
use redirect::*;

const SHELL_BUILTIN_COMMANDS: [&'static str; 6] = ["echo", "type", "exit", "pwd", "cd", "history"];

fn shared_prefix_len(lhs: &str, rhs: &str) -> usize {
    let len = lhs.len().min(rhs.len());
    for i in 0..len {
        if lhs[i..=i] != rhs[i..=i] {
            return i;
        }
    }

    len
}

#[derive(Clone, Debug)]
struct CommandWithContext {
    cmd: Command,
    stdout_redirect: MaybeRedirect,
    stderr_redirect: MaybeRedirect,
}

#[derive(Debug)]
struct PipedCommands(Vec<CommandWithContext>);

impl PipedCommands {
    fn new_invalid() -> Self {
        Self(vec![CommandWithContext {
            cmd: Command::Invalid,
            stdout_redirect: None,
            stderr_redirect: None,
        }])
    }
}

fn parse_command(raw: &str) -> PipedCommands {
    let mut raw_cmds = match ArgParser::new(raw).parse() {
        Some(v) => v.0,
        None => {
            return PipedCommands(vec![CommandWithContext {
                cmd: Command::Invalid,
                stdout_redirect: None,
                stderr_redirect: None,
            }])
        }
    };

    let mut cmds_with_context = vec![];
    while !raw_cmds.is_empty() {
        let raw_cmd = raw_cmds.remove(0);
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
                Command::HistorySave(raw_cmd.args[1].clone().into())
            } else {
                return PipedCommands::new_invalid();
            }
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

        cmds_with_context.push(CommandWithContext {
            cmd,
            stdout_redirect: raw_cmd.stdout_redirect,
            stderr_redirect: raw_cmd.stderr_redirect,
        });
    }

    PipedCommands(cmds_with_context)
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

fn output(
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

impl CustomRLCandidate {
    fn new(mut word: String, is_prefix_only: bool) -> Self {
        if !is_prefix_only {
            word.push(' ');
        }

        Self { word }
    }
}

#[derive(Helper, Validator, Highlighter, Hinter)]
struct CustomRLCompleter {
    executable_names: BTreeSet<String>,
    is_second_update: Cell<bool>,
    options: Cell<Vec<String>>,
}

impl CustomRLCompleter {
    fn matching_names(&self, prefix: &str) -> (Vec<String>, String) {
        let mut options = vec![];
        let mut is_first_match = true;
        let mut shared_prefix = "";

        for name in &self.executable_names {
            if name.starts_with(&prefix) {
                options.push(name.clone());

                if is_first_match {
                    is_first_match = false;
                    shared_prefix = name.as_str();
                } else {
                    let shared_len = shared_prefix_len(shared_prefix, &name);
                    if shared_len < shared_prefix.len() {
                        shared_prefix = &shared_prefix[0..shared_len];
                    }
                }
            }
        }

        (options, shared_prefix.into())
    }

    fn update_single_match(
        &self,
        line: &mut LineBuffer,
        start: usize,
        elected: &str,
        cl: &mut Changeset,
    ) {
        let end = line.pos();
        line.replace(start..end, elected, cl);
    }

    fn update_multiple_match(
        &self,
        options: Vec<String>,
        line: &mut LineBuffer,
        start: usize,
        elected: &str,
        cl: &mut Changeset,
    ) {
        if self.is_second_update.get() {
            let mut is_first = true;
            for name in &options {
                if is_first {
                    io::stdout().write(b"\n\r").unwrap();
                    is_first = false;
                } else {
                    io::stdout().write_all(b"  ").unwrap();
                }
                io::stdout().write_all(name.as_bytes()).unwrap();
            }

            io::stdout().write(b"\n\r$ ").unwrap();
        } else {
            io::stdout().write(&[7]).unwrap();
        }

        io::stdout().flush().unwrap();
        self.is_second_update.set(true);

        let end = line.pos();
        line.replace(start..end, elected, cl);
    }
}

impl Completer for CustomRLCompleter {
    type Candidate = CustomRLCandidate;

    fn complete(
        &self,
        line: &str,
        _pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        // Reset memory.
        self.is_second_update.set(false);
        let (matching_names, longest_shared_prefix) = self.matching_names(line);

        self.options.set(matching_names.clone());

        let completions = if matching_names.len() > 1 {
            if longest_shared_prefix.len() > line.len() {
                // Multiple matches + a better shared prefix:
                vec![
                    CustomRLCandidate::new(longest_shared_prefix.clone(), true),
                    CustomRLCandidate::new(longest_shared_prefix, true),
                ]
            } else {
                // Multiple matches - no better prefix.
                vec![
                    CustomRLCandidate::new(longest_shared_prefix.clone(), true),
                    CustomRLCandidate::new(longest_shared_prefix, true),
                ]
            }
        } else {
            // One or zero match.
            matching_names
                .into_iter()
                .map(|name| CustomRLCandidate::new(name.clone(), false))
                .collect()
        };

        return Ok((0, completions));
    }

    fn update(&self, line: &mut LineBuffer, start: usize, elected: &str, cl: &mut Changeset) {
        let options = self.options.take();
        self.options.set(options.clone());

        if options.len() <= 1 {
            self.update_single_match(line, start, elected, cl);
        } else {
            self.update_multiple_match(options, line, start, elected, cl);
        }
    }
}

impl CustomRLCompleter {
    fn new(env_path_executable_names: Vec<String>) -> Self {
        let mut executable_names = BTreeSet::new();

        for name in SHELL_BUILTIN_COMMANDS {
            executable_names.insert(name.to_string());
        }

        for name in env_path_executable_names {
            executable_names.insert(name);
        }

        Self {
            executable_names,
            is_second_update: Cell::new(false),
            options: Cell::new(vec![]),
        }
    }
}

fn preload_exec_names(env_paths: &Vec<PathBuf>) -> Vec<String> {
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

enum ExecutionResult {
    None,
    Child(Child),
    ChildWithOutputHandling(Child, Option<Redirect>, Option<Redirect>),
}

fn execute_command(
    cmd_with_ctx: CommandWithContext,
    rl: &mut Editor<CustomRLCompleter, DefaultHistory>,
    env_paths: &Vec<PathBuf>,
    original_input: &String,
    pipe_reader: Option<io::PipeReader>,
    pipe_writer: Option<io::PipeWriter>,
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
            let _ = rl.save_history("history.txt");
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
                    _ => output_error(format!("{}: not found", what), cmd_with_ctx.stderr_redirect),
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
                    Ok(child) => ExecutionResult::Child(child),
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
                return ExecutionResult::ChildWithOutputHandling(
                    child,
                    cmd_with_ctx.stdout_redirect,
                    cmd_with_ctx.stderr_redirect,
                );
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
        Command::HistorySave(path) => {
            save_history(path, rl);
            output(String::new(), cmd_with_ctx.stdout_redirect, pipe_writer);
            output_error(String::new(), cmd_with_ctx.stderr_redirect);
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

fn append_to_history(path: String, rl: &mut Editor<CustomRLCompleter, DefaultHistory>) {
    let f = fs::File::open(path).unwrap();
    for line in io::BufReader::new(f).lines() {
        if let Ok(line) = line {
            if !line.is_empty() {
                rl.add_history_entry(line).unwrap();
            }
        }
    }
}

fn save_history(path: String, rl: &mut Editor<CustomRLCompleter, DefaultHistory>) {
    let mut f = fs::File::create(path).unwrap();
    for line in rl.history().iter() {
        f.write_all(line.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
    }
}

fn main() {
    let mut env_vars: HashMap<String, String> = HashMap::new();
    for (k, v) in std::env::vars() {
        env_vars.insert(k, v);
    }

    let env_paths = env_vars
        .get("PATH")
        .map(|v| std::env::split_paths(v).collect())
        .unwrap_or(vec![]);

    let rl_completer = CustomRLCompleter::new(preload_exec_names(&env_paths));
    let mut rl: Editor<CustomRLCompleter, DefaultHistory> = Editor::new().unwrap();

    rl.set_helper(Some(rl_completer));
    let _ = rl.load_history("history.txt");

    loop {
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
        let mut piped_cmds = parse_command(buf.trim()).0;

        let mut pipe_reader: Option<io::PipeReader> = None;
        let mut pipe_writer: Option<io::PipeWriter> = None;

        while !piped_cmds.is_empty() {
            let cmd_with_ctx = piped_cmds.remove(0);
            let (pr, pw) = io::pipe().expect("Failed_making pipe");

            if !piped_cmds.is_empty() {
                pipe_writer = Some(pw);
            }

            let result = execute_command(
                cmd_with_ctx,
                &mut rl,
                &env_paths,
                &buf,
                pipe_reader.take(),
                pipe_writer.take(),
            );
            exec_results.push(result);

            pipe_reader = Some(pr);
        }

        for exec_result in exec_results {
            match exec_result {
                ExecutionResult::Child(mut child) => {
                    child.wait().unwrap();
                }
                ExecutionResult::ChildWithOutputHandling(
                    child,
                    stdout_redirect,
                    stderr_redirect,
                ) => {
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
                ExecutionResult::None => {}
            };
        }
    }
}
