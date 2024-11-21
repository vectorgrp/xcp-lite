use crate::daemon::error::DaemonizationError;
use crate::daemon::IoError;
use std::io::Write;

#[cfg(unix)]
pub mod unixutils {
    use super::*;

    use crate::daemon::platform::unixdeps::{dup2, Facility, Formatter3164, RawFd};

    #[inline]
    // Helper function to use syslog for logging
    pub(crate) fn setup_syslog(name: &str) {
        let formatter = Formatter3164 {
            facility: Facility::LOG_DAEMON,
            hostname: None,
            process: name.to_string(),
            pid: 0,
        };

        let logger = syslog::unix(formatter).expect("Failed to connect to syslog");
        log::set_boxed_logger(Box::new(syslog::BasicLogger::new(logger))).expect("Failed to set logger");
        log::set_max_level(log::LevelFilter::Info);
    }

    #[inline]
    // Helper function to redirect file descriptors (stdin, stdout, stderr) to /dev/null
    pub(crate) unsafe fn redirect_fd(source: RawFd, target: RawFd) -> Result<(), DaemonizationError> {
        if dup2(source, target) < 0 {
            Err(DaemonizationError::Dup2Failed(IoError::last_os_error()))
        } else {
            Ok(())
        }
    }

    #[inline]
    // Helper function to create a PID file
    pub(crate) fn open_pid_file(name: &str, pid: &i32) -> Result<(), DaemonizationError> {
        let path = format!("/var/run/{}.pid", name);
        let mut file = std::fs::File::create(&path).map_err(|_| DaemonizationError::OpenPidFileFailed)?;
        file.write_all(pid.to_string().as_bytes()).map_err(|_| DaemonizationError::WriteToPidFileFailed)?;
        Ok(())
    }

    #[inline]
    // Helper function to remove a PID file
    pub(crate) fn remove_pid_file(name: &str) -> Result<(), DaemonizationError> {
        let path = format!("/var/run/{}.pid", name);
        std::fs::remove_file(&path).map_err(|_| DaemonizationError::ClosePidFileFailed)?;
        Ok(())
    }
}
