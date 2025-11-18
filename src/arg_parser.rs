use crate::redirect::{MaybeRedirect, Redirect};

#[derive(PartialEq, Eq)]
enum CommandParseState {
    DropSection,
    WordSection,
    SingleQuoteSection,
    DoubleQuoteSection,
}

pub(crate) struct UnidentifiedCommand {
    pub(crate) name: String,
    pub(crate) args: Vec<String>,
    pub(crate) stdout_redirect: MaybeRedirect,
    pub(crate) stderr_redirect: MaybeRedirect,
}

pub(crate) struct ArgParser {
    chars: Vec<char>,
    i: usize,
    buf: String,
    state: CommandParseState,
}

impl ArgParser {
    pub(crate) fn new(raw: &str) -> Self {
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

    pub(crate) fn parse(mut self) -> Option<UnidentifiedCommand> {
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
            Some(UnidentifiedCommand {
                name: "".into(),
                args: vec![],
                stdout_redirect: None,
                stderr_redirect: None,
            })
        } else {
            let name = parts.remove(0);
            Some(ArgParser::build_unidentified_command(name, parts))
        }
    }

    fn build_unidentified_command(name: String, mut args: Vec<String>) -> UnidentifiedCommand {
        let mut stdout_redirect = None;
        let mut stderr_redirect = None;

        let mut i = 0usize;
        while i + 1 < args.len() {
            if args[i] == ">" || args[i] == "1>" {
                stdout_redirect = Some(Redirect {
                    filename: args.remove(i + 1),
                    is_append: false,
                });
                args.remove(i);
            } else if args[i] == "2>" {
                stderr_redirect = Some(Redirect {
                    filename: args.remove(i + 1),
                    is_append: false,
                });
                args.remove(i);
            } else if args[i] == ">>" || args[i] == "1>>" {
                stdout_redirect = Some(Redirect {
                    filename: args.remove(i + 1),
                    is_append: true,
                });
                args.remove(i);
            } else if args[i] == "2>>" {
                stderr_redirect = Some(Redirect {
                    filename: args.remove(i + 1),
                    is_append: true,
                });
                args.remove(i);
            } else {
                i += 1;
            }
        }

        UnidentifiedCommand {
            name,
            args,
            stdout_redirect,
            stderr_redirect,
        }
    }
}
