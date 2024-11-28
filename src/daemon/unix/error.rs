use log::SetLoggerError;
use nix::Error as NixError;
use std::io::Error as IoError;
use syslog::Error as SyslogError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonizationError {
    #[error("NixError occurred: {0}")]
    NixError(#[from] NixError),
    #[error("IoError occurred: {0}")]
    IoError(#[from] IoError),
    #[error("SyslogError occurred: {0}")]
    SyslogError(#[from] SyslogError),
    #[error("SetLoggerError occurred: {0}")]
    SetLoggerError(#[from] SetLoggerError),
}
