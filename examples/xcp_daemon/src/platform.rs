#![cfg(unix)]

use xcp::*;
use xcp_type_description::prelude::*;

use serde::{Deserialize, Serialize};
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    iterator::Signals,
};
use std::{thread, time::Duration};
use thiserror::Error;
