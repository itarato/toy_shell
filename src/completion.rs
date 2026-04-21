use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    rc::Rc,
};

use rustyline::{
    completion::{Completer, FilenameCompleter, Pair},
    Helper, Highlighter, Hinter, Validator,
};

use crate::common::{parse_command_name, SHELL_BUILTIN_COMMANDS};

fn run_completion_script(path: &str) -> Vec<String> {
    let output = std::process::Command::new(path)
        .output()
        .expect("Failed to execute completion script");
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().map(|s| s.trim().to_string()).collect()
}

#[derive(Helper, Validator, Highlighter, Hinter)]
pub(crate) struct BinaryAndFileCompleter {
    executable_names: BTreeSet<String>,
    filename_completer: FilenameCompleter,
    completions: Rc<RefCell<HashMap<String, String>>>,
}

impl BinaryAndFileCompleter {
    pub(crate) fn new(
        env_path_executable_names: Vec<String>,
        completions: Rc<RefCell<HashMap<String, String>>>,
    ) -> Self {
        let mut executable_names = BTreeSet::new();

        for name in SHELL_BUILTIN_COMMANDS {
            executable_names.insert(name.to_string());
        }

        for name in env_path_executable_names {
            executable_names.insert(name);
        }

        Self {
            executable_names,
            filename_completer: FilenameCompleter::new(),
            completions,
        }
    }
}

impl Completer for BinaryAndFileCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        if line.contains(' ') {
            let command_name = parse_command_name(line);
            if let Some(completion_script) = self.completions.borrow().get(command_name) {
                let options = run_completion_script(completion_script);

                let custom_candidates = options
                    .iter()
                    .filter_map(|name| {
                        Some(Pair {
                            display: name.to_string(),
                            replacement: format!("{} {} ", command_name, name),
                        })
                    })
                    .collect();

                return Ok((0, custom_candidates));
            }

            return self.filename_completer.complete(line, pos, ctx).map(
                |(new_pos, candidates)| {
                    (
                        new_pos,
                        candidates
                            .into_iter()
                            .map(|pair| Pair {
                                display: fix_path_display(&pair.display),
                                replacement: fix_path_ending(&pair.replacement),
                            })
                            .collect(),
                    )
                },
            );
        }

        let binary_candidates = self
            .executable_names
            .iter()
            .filter_map(|name| {
                if name.starts_with(line) {
                    Some(Pair {
                        display: name.to_string(),
                        replacement: format!("{} ", name),
                    })
                } else {
                    None
                }
            })
            .collect();

        return Ok((0, binary_candidates));
    }
}

fn fix_path_ending(path: &str) -> String {
    if path.ends_with("/") {
        path.to_string()
    } else {
        format!("{} ", path)
    }
}

fn fix_path_display(path: &str) -> String {
    if std::path::Path::new(path).is_dir() {
        format!("{}/", path)
    } else {
        path.to_string()
    }
}
