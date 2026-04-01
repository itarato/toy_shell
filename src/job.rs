use std::process::Child;

pub(crate) struct Jobs(Vec<Job>);

impl Jobs {
    pub(crate) fn new() -> Self {
        Self(vec![])
    }

    pub(crate) fn clean_up(&mut self) {
        self.0.retain_mut(|job| match job.child.try_wait() {
            Err(_) => false,
            Ok(Some(_)) => false,
            Ok(None) => true,
        });
    }

    pub(crate) fn push(&mut self, child: Child, command: String) {
        self.0.push(Job { child, command });
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn print_status_report(&self) {
        for (i, job) in self.0.iter().enumerate() {
            let mark = if i + 1 == self.0.len() {
                "+"
            } else if i + 2 == self.0.len() {
                "-"
            } else {
                " "
            };
            println!("[{}]{}  {:<24}{}", i + 1, mark, "Running", job.command);
        }
    }
}

pub(crate) struct Job {
    child: Child,
    command: String,
}
