#![cfg(unix)]

pub use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    iterator::Signals,
};
pub use std::{thread, time::Duration};
pub use thiserror::Error;
pub use xcp::*;
