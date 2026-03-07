use std::collections::BTreeSet;

use rustyline::{
    completion::{Completer, FilenameCompleter, Pair},
    Helper, Highlighter, Hinter, Validator,
};

use crate::common::SHELL_BUILTIN_COMMANDS;

#[derive(Helper, Validator, Highlighter, Hinter)]
pub(crate) struct BinaryAndFileCompleter {
    executable_names: BTreeSet<String>,
    filename_completer: FilenameCompleter,
}

impl BinaryAndFileCompleter {
    pub(crate) fn new(env_path_executable_names: Vec<String>) -> Self {
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
            return self.filename_completer.complete(line, pos, ctx).map(
                |(new_pos, candidates)| {
                    (
                        new_pos,
                        candidates
                            .into_iter()
                            .map(|pair| Pair {
                                display: pair.display,
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
