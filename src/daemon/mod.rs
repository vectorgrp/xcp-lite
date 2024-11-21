pub mod process;
pub use process::Process;

pub mod error;
pub use error::*;

mod platform;
#[cfg(unix)]
use platform::unixdeps::{
    chdir, fork, get_group_by_name, get_user_by_name, geteuid, getpid, setgid, setsid, setuid, umask, CString, IntoRawFd, PermissionsExt, STDERR_FILENO, STDIN_FILENO, STDOUT_FILENO,
};

mod utils;
#[cfg(unix)]
use utils::unixutils::*;

mod config;
pub use config::*;

use log::{error, info};
use std::fs::{create_dir_all, set_permissions, File, Permissions};
use std::io::Error as IoError;
use std::path::Path;

pub struct Daemon<P: Process> {
    pid: i32,
    sid: i32,
    name: &'static str,
    process: P,
}

#[cfg(unix)]
pub mod unix {
    use super::*;

    impl<P: Process> Daemon<P> {
        pub fn new(process: P, name: &'static str) -> Daemon<P> {
            Daemon { pid: 0, sid: 0, name, process }
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

        pub fn pid(&self) -> i32 {
            self.pid
        }

        pub fn sid(&self) -> i32 {
            self.sid
        }

        fn daemonize(&mut self) -> Result<(), DaemonizationError> {
            // Fork the parent process. Fork is called
            // both in the parent and child process.
            // SAFETY:
            // 1. We immediately validate the return value
            // 2. We have explicit error handling for negative (failed) fork scenarios
            // 3. We create separate code paths for child and parent processes
            match unsafe { fork() } {
                0 => {
                    // In the child process, fork returns 0 and not the actual pid of the caller
                    // To get the actual PID of the child process, getpid() is called
                    // SAFETY:
                    // 1. The kernel guarantees a unique, non-zero PID for each process
                    // 2. We immediately assert the PID is valid
                    self.pid = unsafe { getpid() };
                    assert!(self.pid > 0);

                    // Setup logger for the child process
                    setup_syslog(self.name);

                    // Allow the process to create files and directories (set umask)
                    // SAFETY: The call has no failure mode that can corrupt memory or state
                    unsafe { umask(0) };

                    // Create a PID file for the daemon
                    open_pid_file(&self.name, &self.pid)?;

                    // Set the child process as the session leader
                    // SAFETY:
                    // 1. It cannot create invalid process group states
                    // 2. We explicitly check for error conditions (negative return)
                    // 3. The kernel manages session and process group integrity
                    self.sid = unsafe { setsid() };
                    if self.sid < 0 {
                        error!("Failed to set SID for the child process");
                        return Err(DaemonizationError::SetsidFailed);
                    }

                    // Change the working directory to root, so the daemon is not locked to a directory
                    let cstr_name = CString::new("/").expect("CString::new failed");
                    let cstr_name = cstr_name.as_ptr();
                    // SAFETY:
                    // 1. We use a valid, null-terminated string for the root path ("/")
                    // 2. Failure is detected through the return value check
                    // 3. Changing directory cannot corrupt memory or process state
                    if unsafe { chdir(cstr_name) } < 0 {
                        error!("Failed to change working directory to root");
                        return Err(DaemonizationError::ChdirFailed);
                    }

                    // Open /dev/null and redirect stdin, stdout, and stderr to it (detach from terminal)
                    let dn = if let Ok(file) = File::open("/dev/null") {
                        file.into_raw_fd()
                    } else {
                        error!("Failed to open /dev/null file descriptor");
                        return Err(DaemonizationError::OpenDevNullFailed);
                    };

                    // Perform redirection of stdin, stdout, and stderr to /dev/null
                    // SAFETY:
                    // 1. We've validated the /dev/null file descriptor before use
                    // 2. redirect_fd() includes error handling to prevent invalid states
                    // 3. Redirecting standard I/O streams cannot cause memory unsafety
                    unsafe {
                        redirect_fd(dn, STDIN_FILENO)?; // Redirect stdin to /dev/null
                        redirect_fd(dn, STDOUT_FILENO)?; // Redirect stdout to /dev/null
                        redirect_fd(dn, STDERR_FILENO)?; // Redirect stderr to /dev/null

                        // Close the /dev/null file descriptor as it's no longer needed
                        /* if close(dn) < 0 {
                            error!("Failed to close /dev/null file descriptor");
                            return Err(DaemonizationError::FailedToCloseFd(IoError::last_os_error()));
                        } else {
                            return Ok(()); // Successfully daemonized
                        } */
                    }

                    // self.drop_privileges();
                    info!("Process '{}' daemonized with PID: {}", self.name, self.pid);
                    Ok(())
                }
                pid if pid > 0 => {
                    // In the parent, fork returns a positive number
                    // representing the pid of thie child process
                    // i.e: If the pid is positive, we are in
                    // the parent process and it should exit
                    std::process::exit(0);
                }
                _ => {
                    // If fork returns a negative number, it means the fork failed
                    error!("Failed to fork the parent process");
                    return Err(DaemonizationError::ForkFailed);
                }
            }
        }

        #[allow(dead_code)]
        fn drop_privileges(&self) -> Result<(), DaemonizationError> {
            // Ensure we're running as root before attempting to drop privileges
            if unsafe { geteuid() } != 0 {
                error!("No root privileges to drop");
                return Err(DaemonizationError::NotRunningAsRoot);
            }

            // Get user and group IDs
            let daemon_user = get_user_by_name(USER).ok_or(DaemonizationError::UserNotFound)?;
            let daemon_group = get_group_by_name(GROUP).ok_or(DaemonizationError::GroupNotFound)?;

            // Drop group privileges first
            if unsafe { setgid(daemon_group.gid()) } != 0 {
                error!("Failed to drop group privileges");
                return Err(DaemonizationError::PrivilegeDropFailed);
            }

            // Then drop user privileges
            if unsafe { setuid(daemon_user.uid()) } != 0 {
                error!("Failed to drop user privileges");
                return Err(DaemonizationError::PrivilegeDropFailed);
            }

            info!("Dropped root privileges");
            Ok(())
        }

        #[allow(dead_code)]
        fn setup_directories(&self) -> Result<(), DaemonizationError> {
            // Create runtime directory if it doesn't exist

            let runtime_dir = Path::new(RUNTIME_DIR);
            if !runtime_dir.exists() {
                create_dir_all(runtime_dir).map_err(|e| DaemonizationError::DirectoryCreationFailed(e))?;
            }

            // Create data directory if it doesn't exist
            let data_dir = Path::new(DATA_DIR);
            if !data_dir.exists() {
                create_dir_all(data_dir).map_err(|e| DaemonizationError::DirectoryCreationFailed(e))?;
            }

            // Set proper permissions (typically 755 for dirs)
            let dir_perms = Permissions::from_mode(0o755);
            set_permissions(runtime_dir, dir_perms.clone()).map_err(|e| DaemonizationError::PermissionSetFailed(e))?;
            set_permissions(data_dir, dir_perms).map_err(|e| DaemonizationError::PermissionSetFailed(e))?;

            Ok(())
        }
    }

    impl<P: Process> Drop for Daemon<P> {
        fn drop(&mut self) {
            // Remove the PID file when the daemon is dropped
            if self.pid > 0 {
                if let Err(e) = remove_pid_file(&self.name) {
                    error!("Failed to remove PID file: {}", e);
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use thiserror::Error;

        use super::*;
        use libc::{SIGHUP, SIGINT, SIGTERM};

        #[derive(Error, Debug)]
        pub enum TestProcessError {
            #[error("An error occurred: {0}")]
            GeneralError(String),
        }
        struct TestProcess {
            config: DaemonConfig,
        }

        impl TestProcess {
            fn new(config: DaemonConfig) -> Self {
                TestProcess { config }
            }

            fn handle_signal(&self, signal: i32) {
                match signal {
                    SIGINT => info!("Received SIGINT signal"),
                    SIGTERM => {
                        info!("Received SIGTERM signal");
                        std::process::exit(0);
                    }
                    SIGHUP => {
                        info!("Received SIGHUP signal");
                        for section in self.config.sections() {
                            for item in section.items {
                                info!("{} = {}", item.0, item.1);
                            }
                        }
                    }
                    _ => info!("Received unknown signal"),
                }
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

                let mut signals = signal_hook::iterator::Signals::new(&[SIGINT, SIGTERM, SIGHUP]).map_err(|e| TestProcessError::GeneralError(e.to_string()))?;

                let timer = 60; // Seconds
                for i in 0..timer {
                    for signal in signals.pending() {
                        self.handle_signal(signal);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    info!("TestProcess running for {} more seconds", timer - i);
                }
                Ok(())
            }

            fn deinit(&mut self) -> Result<(), Self::Error> {
                info!("TestProcess deinit");
                Ok(())
            }
        }

        #[test]
        fn test_daemon() {
            let mut daemon = Daemon::new(TestProcess::new(DaemonConfig::default()), "TestProcess");
            daemon.run().unwrap();
            let pid = daemon.pid();
            let sid = daemon.sid();

            assert!(pid > 0);
            assert!(sid > 0);
        }
    }
}
