use std::{
    cell::Cell,
    collections::BTreeSet,
    io::{self, Write},
};

use rustyline::{
    completion::{Candidate, Completer},
    line_buffer::LineBuffer,
    Changeset, Helper, Highlighter, Hinter, Validator,
};

use crate::common::SHELL_BUILTIN_COMMANDS;

fn shared_prefix_len(lhs: &str, rhs: &str) -> usize {
    let len = lhs.len().min(rhs.len());
    for i in 0..len {
        if lhs[i..=i] != rhs[i..=i] {
            return i;
        }
    }

    len
}

pub(crate) struct CustomRLCandidate {
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
pub(crate) struct CustomRLCompleter {
    executable_names: BTreeSet<String>,
    is_second_update: Cell<bool>,
    options: Cell<Vec<String>>,
}

impl CustomRLCompleter {
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
            is_second_update: Cell::new(false),
            options: Cell::new(vec![]),
        }
    }

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
