pub mod process;

pub use process::*;

pub mod error;
pub use error::*;

mod config;
pub use config::*;

use nix::{
    fcntl::{open, OFlag},
    libc::{STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO},
    sys::stat::{self, Mode},
    unistd::{self, fork, getpid, setsid, ForkResult, Pid},
};

use log::{error, info};
use syslog::{Facility, Formatter3164};
use std::{fs::File, io::Write, os::fd::AsRawFd, path::Path};

pub struct Daemon<P: Process> {
    process: P,
    pid: Pid,
    sid: Pid,
}

impl<P: Process> Daemon<P> {
    pub fn new(process: P) -> Daemon<P> {
        Self {
            process: process,
            pid: Pid::from_raw(-1),
            sid: Pid::from_raw(-1),
        }
    }

    pub fn run(&mut self) -> Result<(), P::Error> {
        match self.daemonize() {
            Ok(_) => {
                self.process.init()?;
                self.process.run()?;
                self.process.deinit()?;
                Ok(())
            }
            Err(e) => {
                error!("Failed to daemonize: {}", e);
                panic!("Failed to daemonize: {}", e);
            }
        }
    }

    fn daemonize(&mut self) -> Result<(), DaemonizationError> {
        match unsafe { fork()? } {
            ForkResult::Parent { child: _ } => {
                std::process::exit(0);
            }
            ForkResult::Child => {
                self.setup_syslog()?;
                self.pid = getpid();
                self.sid = setsid()?;
                self.set_cwd()?;
                stat::umask(Mode::empty());
                self.create_pid_file()?;
                self.redirect_stdio()?;
                info!("Process '{}' daemonized with PID: {}", self.process.config().name(), self.pid);
            }
        }

        Ok(())
    }

    fn set_cwd(&mut self) -> Result<(), DaemonizationError> {
        unistd::chdir(Path::new(self.process.config().workdir()))?;
        Ok(())
    }

    fn create_pid_file(&self) -> Result<(), DaemonizationError> {
        let path = Path::new(self.process.config().pid_fpath());
        let mut pid_file = File::create(path)?;
        write!(pid_file, "{}", self.pid)?;
        let permissions = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH;
        nix::sys::stat::fchmod(pid_file.as_raw_fd(), permissions)?;
        Ok(())
    }

    fn redirect_stdio(&self) -> Result<(), DaemonizationError> {
        let log_path = Path::new(self.process.config().logdir());

        // Create the log file if it does not exist
        if !log_path.exists() {
            File::create(log_path)?;
        }

        let dst = open(log_path, OFlag::O_RDWR, Mode::empty())?;

        for src in &[STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO] {
            unistd::dup2(dst, *src)?;
        }

        unistd::close(dst)?;

        Ok(())
    }

    fn setup_syslog(&self) -> Result<(), DaemonizationError> {
        let formatter = Formatter3164 {
            facility: Facility::LOG_DAEMON,
            hostname: None,
            process: self.process.config().name().to_string(),
            pid: 0,
        };

        let logger = syslog::unix(formatter)?;
        log::set_boxed_logger(Box::new(syslog::BasicLogger::new(logger)))?;
        log::set_max_level(self.process.config().loglvl());

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum TestProcessError {}

    struct TestProcess {
        cfg: ProcessConfig,
    }

    impl TestProcess {
        pub fn new(cfg: ProcessConfig) -> Self {
            Self { cfg }
        }
    }

    impl Process for TestProcess {
        type Error = TestProcessError;

        fn init(&mut self) -> Result<(), Self::Error> {
            info!("TestProcess init");
            Ok(())
        }

        fn run(&mut self) -> Result<(), Self::Error> {
            info!("TestProcess run");
            Ok(())
        }

        fn deinit(&mut self) -> Result<(), Self::Error> {
            info!("TestProcess deinit");
            Ok(())
        }

        fn config(&self) -> &ProcessConfig {
            &self.cfg
        }
    }

    #[test]
    fn test_daemon() {
        let cfg = ProcessConfig::default();
        let mut daemon = Daemon::new(TestProcess::new(cfg));
        daemon.run().unwrap();
    }
}
