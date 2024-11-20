// Unix specific dependencies

#[cfg(unix)]
pub use libc::{fork, setsid, chdir, getpid, umask, STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO};
#[cfg(unix)]
pub use syslog::{Facility, Formatter3164};
#[cfg(unix)]
pub use std::ffi::CString;
#[cfg(unix)]
pub use std::os::unix::io::IntoRawFd;