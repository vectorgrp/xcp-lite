//-----------------------------------------------------------------------------
// Crate xcp
// Path: src/lib.rs
// xcp is a library crate that provides an XCP on ETH implementation, calibration data segment handling and registry functionality.

// Note that the tests can not be executed in parallel
// Use cargo test -- --test-threads=1 --nocapture

// #![allow(non_upper_case_globals)]
// #![allow(non_camel_case_types)]
// #![allow(non_snake_case)]
// #![allow(unused_variables)]
// #![allow(unused_imports)]
// #![allow(dead_code)]

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp"
#![crate_name = "xcp"]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

//-----------------------------------------------------------------------------

// Submodule xcp
mod xcp;
pub use xcp::Xcp;
pub use xcp::XcpBuilder;
pub use xcp::XcpCalPage;
pub use xcp::XcpEvent;
pub use xcp::XcpLogLevel;
pub use xcp::XcpTransportLayer;

// Submodule cal
mod cal;
pub use cal::CalPageField;
pub use cal::CalPageTrait;
pub use cal::CalSeg;
pub use cal::CalSegTrait;

// Submodule daq
mod daq;
pub use daq::DaqEvent;

// Submodule reg
mod reg;
pub use reg::RegDataTypeHandler;
pub use reg::RegDataTypeProperties;
pub use reg::RegistryCharacteristic;
pub use reg::RegistryDataType;
pub use reg::RegistryMeasurement;

// @@@@ Reexport for integration tests
pub use xcp::xcp_test::test_reinit;

// XCPlite FFI bindings
mod xcplib {
    include!("xcplite.rs");
}

//----------------------------------------------------------------------------------------------
// Manually register a static calibration variable

#[macro_export]
macro_rules! cal_register {
    (   $name:ident ) => {{
        let name = stringify!($name);
        let datatype = unsafe { $name.get_type() };
        let addr = unsafe { &($name) as *const _ as u64 };
        let c = RegistryCharacteristic::new(None, name.to_string(), datatype, "", datatype.get_min(), datatype.get_max(), "", 1, 1, addr);
        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
    }};
}

//-----------------------------------------------------------------------------
// XCP println macro

// Print formated test to CANape console
#[allow(unused_macros)]
#[macro_export]
macro_rules! xcp_println {
    ( $fmt:expr ) => {
        Xcp::print(&format!($fmt));
    };
    ( $fmt:expr, $( $arg:expr ),* ) => {
        Xcp::print(&format!($fmt, $( $arg ),*));
    };
}
