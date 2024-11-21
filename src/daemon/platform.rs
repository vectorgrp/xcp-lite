// Unix specific dependencies
#[cfg(unix)]
pub mod unixdeps {
    pub use libc::{chdir, fork, geteuid, getpid, setgid, dup2, setsid, setuid, umask, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO};
    pub use std::ffi::CString;
    pub use std::os::unix::fs::PermissionsExt;
    pub use std::os::unix::io::{IntoRawFd, RawFd};
    pub use syslog::{Facility, Formatter3164};
    pub use users::{get_group_by_name, get_user_by_name};
}
