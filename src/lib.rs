//-----------------------------------------------------------------------------
// Crate xcp
// Path: src/lib.rs
// xcp is a library crate that provides an XCP on ETH implementation, calibration data segment handling and registry functionality.

// Note that the tests can not be executed in parallel
// Use cargo test -- --test-threads=1 --nocapture

//#![warn(missing_docs)]

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp"
#![crate_name = "xcp"]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

//-----------------------------------------------------------------------------

// Submodule xcp
mod xcp;
pub use xcp::cal::cal_seg::CalPageField;
pub use xcp::cal::cal_seg::CalSeg;
pub use xcp::daq::daq_event::DaqEvent;
pub use xcp::Xcp;
pub use xcp::XcpBuilder;
pub use xcp::XcpCalPage;
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

// XCPlite FFI bindings
mod xcplib {
    include!("xcplite.rs");
}

//----------------------------------------------------------------------------------------------
// Manually register a static measurement and calibration variables

/// Register a static calibration parameter
#[macro_export]
macro_rules! cal_register_static {
    (   $variable:expr ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, "", datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};
    (   $variable:expr, $comment:expr ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};

    (   $variable:expr, $comment:expr, $unit:expr ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), $unit, 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};
}

/// Register a static measurement variable with
#[macro_export]
macro_rules! daq_register_static {
    (   $variable:expr, $event:ident ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, "", datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};
    (   $variable:expr, $event:ident, $comment:expr ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};

    (   $variable:expr, $event:ident, $comment:expr, $unit:expr ) => {{
        let name = stringify!($variable);
        let datatype = unsafe { ($variable).get_type() };
        let addr = unsafe { &($variable) as *const _ as u64 };
        let mut c = RegistryCharacteristic::new(None, name.to_string(), datatype, $comment, datatype.get_min(), datatype.get_max(), $unit, 1, 1, addr);
        c.set_event($event);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
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
