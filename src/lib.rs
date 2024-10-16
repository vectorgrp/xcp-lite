#![cfg(not(doctest))]
//#![warn(missing_docs)]

//-----------------------------------------------------------------------------
// Crate xcp
// Path: src/lib.rs

//
// Note that the tests can not be executed in parallel
// Use cargo test -- --test-threads=1 --features=serde --nocapture

//! A lightweight XCP on Ethernet implementation
//! The 'xcp' crate provides an XCP on ETH implementation,a wrapper type for calibration variables and
//! a registry to describe events, meaesurement and calibration objects for A2L generation.
//!
//! ## Example
//!
//! ```
//!
//! use xcp::*;
//! use xcp_type_description::prelude::*;
//!
//! #[derive(XcpTypeDescription)]
//! #[derive(serde::Serialize, serde::Deserialize)]
//! #[derive(Debug, Clone, Copy)]
//! struct CalPage {
//!     #[type_description(comment = "Amplitude")]
//!     #[type_description(unit = "Volt")]
//!     #[type_description(min = "0")]
//!     #[type_description(max = "400")]
//!     ampl: f64,
//!
//!     #[type_description(comment = "Period")]
//!     #[type_description(unit = "s")]
//!     #[type_description(min = "0")]
//!     #[type_description(max = "1000")]
//!     period: f64,
//! }
//!
//!
//! const CAL_PAGE: CalPage = CalPage {
//!     ampl: 100.0,
//!     period: 1.0,
//! };
//!
//! // Initialize XCP
//! let xcp = XcpBuilder::new("xcp_lite").start_server(XcpTransportLayer::Tcp, [127,0,0,1], 5555)?;
//!
//! // Create a calibration segment and auto register its fields as calibration variables
//! let cal_page = xcp.create_calseg("CalPage", &CAL_PAGE);
//!
//! // Create an event
//! let event = daq_create_event!("task1");
//!
//! let mut signal: f64 = 0.0;
//!
//! // Register a variable of basic type to be captured directly from stack
//! daq_register!(signal, event, "", "", 1.0, 0.0);
//!
//! loop {
//!
//!     signal += 0.1;
//!     if signal > cal_page.ampl { signal = 0.0; } // calibration parameter access to ampl
//!
//!     // Trigger event "task1" for data acquisition, reading variable signal from stack happens here
//!     event.trigger();
//!
//!     // Sync the calibration segment with modifications from the XCP client
//!     cal_page.sync();
//! }
//!
//! ```
//!
//!
//!

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp"
#![crate_name = "xcp"]

//-----------------------------------------------------------------------------

// Submodule xcp
mod xcp;
pub use xcp::cal::cal_seg::CalPageField;
pub use xcp::cal::cal_seg::CalSeg;
pub use xcp::daq::daq_event::DaqEvent;
pub use xcp::Xcp;
pub use xcp::XcpBuilder;
pub use xcp::XcpCalPage;
pub use xcp::XcpError;
pub use xcp::XcpEvent;
pub use xcp::XcpLogLevel;
pub use xcp::XcpSessionStatus;
pub use xcp::XcpTransportLayer;

// @@@@ Reexport for integration tests
pub use xcp::xcp_test::test_reinit;

// Submodule reg
mod reg;
pub use reg::RegistryCharacteristic;
pub use reg::RegistryDataType;
pub use reg::RegistryDataTypeTrait;
pub use reg::RegistryMeasurement;

//----------------------------------------------------------------------------------------------
// Manually register a static measurement and calibration variables

/// Register a static calibration parameter
#[macro_export]
macro_rules! cal_register_static {
    (   $variable:expr ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, "", datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};
    (   $variable:expr, $comment:expr ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};

    (   $variable:expr, $comment:expr, $unit:expr ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), $unit, 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};
}

/// Register a static measurement variable
#[macro_export]
macro_rules! daq_register_static {
    (   $variable:expr, $event:ident ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, "", datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};
    (   $variable:expr, $event:ident, $comment:expr ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};

    (   $variable:expr, $event:ident, $comment:expr, $unit:expr ) => {{
        let name = stringify!($variable);
        let datatype = ($variable).get_type();
        let addr = &($variable) as *const _ as u64;
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), $unit, 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
    }};
}

//-----------------------------------------------------------------------------
// XCP println macro

/// Print formated text to CANape console
#[allow(unused_macros)]
#[macro_export]
macro_rules! xcp_println {
    ( $fmt:expr ) => {
        Xcp::get().print(&format!($fmt));
    };
    ( $fmt:expr, $( $arg:expr ),* ) => {
        Xcp::get().print(&format!($fmt, $( $arg ),*));
    };
}
