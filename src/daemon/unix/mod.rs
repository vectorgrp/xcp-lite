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

use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::fd::AsRawFd,
    path::Path,
    sync::Mutex,
};
use syslog::{Facility, Formatter3164, Logger};

pub struct Daemon<P: Process> {
    process: P,
    pid: Pid,
    sid: Pid,
    syslog_logger: Mutex<Logger<syslog::LoggerBackend, Formatter3164>>,
}

impl<P: Process> Daemon<P> {
    pub fn new(process: P) -> Daemon<P> {
        let process_name = process.config().name().to_string();
        Self {
            process,
            pid: Pid::from_raw(-1),
            sid: Pid::from_raw(-1),
            syslog_logger: Mutex::new(
                syslog::unix(Formatter3164 {
                    facility: Facility::LOG_DAEMON,
                    hostname: None,
                    process: process_name,
                    pid: 0,
                })
                .unwrap(),
            ),
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
                self.log_error(&format!("Failed to daemonize: {}", e));
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
                self.log_info(&format!("Process '{}' daemonized with PID: {}", self.process.config().name(), self.pid));
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
        // Ensure stdin path exists
        let stdin_path = Path::new(self.process.config().stdin());
        if !stdin_path.exists() {
            OpenOptions::new().create(true).write(true).append(true).open(stdin_path)?;
        }
        let stdin_dst = open(stdin_path, OFlag::O_RDWR | OFlag::O_APPEND, Mode::empty())?;
        unistd::dup2(stdin_dst, STDIN_FILENO)?;
        unistd::close(stdin_dst)?;

        // Ensure stdout path exists
        let stdout_path = Path::new(self.process.config().stdout());
        if !stdout_path.exists() {
            OpenOptions::new().create(true).write(true).append(true).open(stdout_path)?;
        }
        let stdout_dst = open(stdout_path, OFlag::O_RDWR | OFlag::O_APPEND, Mode::empty())?;
        unistd::dup2(stdout_dst, STDOUT_FILENO)?;
        unistd::close(stdout_dst)?;

        // Ensure stderr path exists
        let stderr_path = Path::new(self.process.config().stderr());
        if !stderr_path.exists() {
            OpenOptions::new().create(true).write(true).append(true).open(stderr_path)?;
        }
        let stderr_dst = open(stderr_path, OFlag::O_RDWR | OFlag::O_APPEND, Mode::empty())?;
        unistd::dup2(stderr_dst, STDERR_FILENO)?;
        unistd::close(stderr_dst)?;

        Ok(())
    }

    fn setup_syslog(&mut self) -> Result<(), DaemonizationError> {
        // In the child we have the pid of the process so
        // we reinitialize the logger with the pid
        let formatter = Formatter3164 {
            facility: Facility::LOG_DAEMON,
            hostname: None,
            process: self.process.config().name().to_string(),
            pid: self.pid.as_raw() as u32,
        };

        let logger = syslog::unix(formatter)?;
        self.syslog_logger = Mutex::new(logger);
        Ok(())
    }

    pub fn log_info(&self, message: &str) {
        self.syslog_logger.lock().unwrap().info(message).unwrap();
    }

    pub fn log_error(&self, message: &str) {
        self.syslog_logger.lock().unwrap().err(message).unwrap();
    }

    pub fn log_warning(&self, message: &str) {
        self.syslog_logger.lock().unwrap().warning(message).unwrap();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use log::info;
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
