use std::process::Child;

pub(crate) struct Jobs(Vec<Job>);

impl Jobs {
    pub(crate) fn new() -> Self {
        Self(vec![])
    }

    pub(crate) fn clean_up(&mut self) {
        self.0.retain_mut(|job| job.status() == JobStatus::Running);
    }

    pub(crate) fn push(&mut self, child: Child, command: String) {
        self.0.push(Job { child, command });
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn print_status_report(&mut self) {
        let jobs_len = self.0.len();
        for (i, job) in self.0.iter_mut().enumerate() {
            let mark = if i + 1 == jobs_len {
                "+"
            } else if i + 2 == jobs_len {
                "-"
            } else {
                " "
            };
            let status = match job.status() {
                JobStatus::Done => "Done",
                JobStatus::Error => "Error",
                JobStatus::Running => "Running",
            };
            let job_mark = match job.status() {
                JobStatus::Done | JobStatus::Error => "",
                JobStatus::Running => " &",
            };
            println!(
                "[{}]{}  {:<24}{}{}",
                i + 1,
                mark,
                status,
                job.command.replace(" &", ""),
                job_mark
            );
        }

        self.clean_up();
    }
}

#[derive(PartialEq, Eq)]
enum JobStatus {
    Running,
    Done,
    Error,
}

pub(crate) struct Job {
    child: Child,
    command: String,
}

impl Job {
    fn status(&mut self) -> JobStatus {
        match self.child.try_wait() {
            Err(_) => JobStatus::Error,
            Ok(Some(_)) => JobStatus::Done,
            Ok(None) => JobStatus::Running,
        }
    }
}
