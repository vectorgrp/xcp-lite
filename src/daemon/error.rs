use std::io::Error as IoError;
use std::fmt;

pub enum DaemonizationError {
    ForkFailed,
    SetsidFailed,
    ChdirFailed,
    RedirectFailed,
    CloseFailed,
    OpenDevNullFailed,
    Dup2Failed(IoError),
    FailedToCloseFd(IoError),
    OpenPidFileFailed,
    ClosePidFileFailed,
    WriteToPidFileFailed,
    NotRunningAsRoot,
    UserNotFound,
    GroupNotFound,
    PrivilegeDropFailed,
    DirectoryCreationFailed(IoError),
    PermissionSetFailed(IoError),
}

impl fmt::Display for DaemonizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonizationError::ForkFailed => write!(f, "Fork failed"),
            DaemonizationError::SetsidFailed => write!(f, "Setsid failed"),
            DaemonizationError::ChdirFailed => write!(f, "Chdir failed"),
            DaemonizationError::RedirectFailed => write!(f, "Redirect failed"),
            DaemonizationError::CloseFailed => write!(f, "Close failed"),
            DaemonizationError::OpenDevNullFailed => write!(f, "Open /dev/null failed"),
            DaemonizationError::Dup2Failed(err) => write!(f, "Dup2 failed: {}", err),
            DaemonizationError::FailedToCloseFd(err) => write!(f, "Failed to close file descriptor: {}", err),
            DaemonizationError::OpenPidFileFailed => write!(f, "Failed to open PID file"),
            DaemonizationError::ClosePidFileFailed => write!(f, "Failed to close PID file"),
            DaemonizationError::WriteToPidFileFailed => write!(f, "Failed to write PID file."),
            DaemonizationError::NotRunningAsRoot => write!(f, "Not running as root"),
            DaemonizationError::UserNotFound => write!(f, "Specified user not found"),
            DaemonizationError::GroupNotFound => write!(f, "Specified group not found"),
            DaemonizationError::PrivilegeDropFailed => write!(f, "Failed to drop privileges"),
            DaemonizationError::DirectoryCreationFailed(err) => write!(f, "Failed to drop privileges: {}", err),
            DaemonizationError::PermissionSetFailed(err) => write!(f, "Failed to set permissions: {}", err)
        }
    }
}

impl fmt::Debug for DaemonizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}