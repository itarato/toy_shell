use std::process::Child;

pub(crate) struct Jobs {
    jobs: Vec<Job>,
    counter: usize,
}

impl Jobs {
    pub(crate) fn new() -> Self {
        Self {
            jobs: vec![],
            counter: 0,
        }
    }

    pub(crate) fn clean_up(&mut self) {
        self.jobs
            .retain_mut(|job| job.status() == JobStatus::Running);
    }

    pub(crate) fn push(&mut self, child: Child, command: String) {
        self.counter += 1;
        self.jobs.push(Job {
            child,
            command,
            index: self.counter,
        });
    }

    pub(crate) fn len(&self) -> usize {
        self.jobs.len()
    }

    pub(crate) fn print_status_report(&mut self, only_recycle: bool) {
        let jobs_len = self.jobs.len();
        for (i, job) in self.jobs.iter_mut().enumerate() {
            if only_recycle && job.status() != JobStatus::Done {
                continue;
            }

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
                job.index,
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
    index: usize,
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
