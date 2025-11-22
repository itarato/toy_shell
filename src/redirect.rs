use std::{fs::File, io};

#[derive(Clone, Debug)]
//                           Path    Append?
pub(crate) struct Redirect {
    pub(crate) filename: String,
    pub(crate) is_append: bool,
}

impl Redirect {
    pub(crate) fn file(&self) -> io::Result<File> {
        std::fs::File::options()
            .write(true)
            .create(true)
            .append(self.is_append)
            .open(&self.filename)
    }
}

pub(crate) type MaybeRedirect = Option<Redirect>;
