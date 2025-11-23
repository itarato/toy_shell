#[derive(Clone, Debug)]
pub(crate) enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(String),
    Unknown(String, Vec<String>),
    Cd(String),
    Pwd,
    History(usize),
    HistoryAppend(String),
    HistorySave(String),
    Empty,
    Invalid,
}

impl Command {
    pub(crate) fn name(&self) -> String {
        match self {
            Command::Exit(_) => "exit".into(),
            Command::Echo(_) => "echo".into(),
            Command::Type(_) => "type".into(),
            Command::History(_) => "history".into(),
            Command::HistoryAppend(_) => "history".into(),
            Command::HistorySave(_) => "history".into(),
            Command::Unknown(name, _) => name.clone(),
            Command::Pwd => "pwd".into(),
            Command::Cd(_) => "cd".into(),
            Command::Empty => "".into(),
            Command::Invalid => unimplemented!(),
        }
    }
}
