#[derive(Clone, Debug)]
pub(crate) enum Command {
    Exit(i32),
    Echo(Vec<String>),
    Type(String),
    Unknown(String, Vec<String>),
    Cd(String),
    Pwd,
    Jobs,
    History(usize),
    HistoryAppend(String),
    CompleteGet(String /* command */),
    CompleteSet {
        script_path: String,
        command: String,
    },
    CompleteRemove(String),
    //          Path    Append?
    HistorySave(String, bool),
    DeclarePrint(String),
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
            Command::HistorySave(_, _) => "history".into(),
            Command::Unknown(name, _) => name.clone(),
            Command::Pwd => "pwd".into(),
            Command::Cd(_) => "cd".into(),
            Command::Empty => "".into(),
            Command::Jobs => "jobs".into(),
            Command::CompleteGet(_) => "complete".into(),
            Command::CompleteSet { .. } => "complete".into(),
            Command::CompleteRemove(_) => "complete".into(),
            Command::Invalid => unimplemented!(),
            Command::DeclarePrint(_) => "declare".into(),
        }
    }
}
