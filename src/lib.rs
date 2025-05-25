//-----------------------------------------------------------------------------
// Crate xcp_lite
// Path: src/lib.rs

//
// Note that the tests can not be executed in parallel
// Use cargo test --features=a2l_reader  -- --test-threads=1 --nocapture

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp_lite"
#![crate_name = "xcp_lite"]
//
// Disabled clippy lints
//#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)] // bindgen
#![allow(dead_code)] // bindgen

// #![warn(clippy::pedantic)]
// #![allow(clippy::doc_markdown)]
// #![allow(clippy::missing_errors_doc)]
// #![allow(clippy::missing_panics_doc)]
// #![allow(clippy::must_use_candidate)]
// #![allow(clippy::uninlined_format_args)]
// #![allow(clippy::module_name_repetitions)]
// #![allow(clippy::struct_field_names)]
// #![allow(clippy::unreadable_literal)]
// #![allow(clippy::if_not_else)]
// #![allow(clippy::wildcard_imports)]
// #![allow(clippy::cast_lossless)]
// #![allow(clippy::ref_as_ptr)]
// #![allow(clippy::ptr_as_ptr)]
// #![allow(clippy::cast_possible_wrap)]
// #![allow(clippy::trivially_copy_pass_by_ref)]
//

//-----------------------------------------------------------------------------

// Submodule xcp
mod xcp;
#[doc(hidden)]
pub use xcp::CalCell;
#[doc(hidden)]
pub use xcp::CalSeg;
#[doc(inline)]
pub use xcp::Xcp;
#[doc(hidden)] // For integration test
pub use xcp::XcpCalPage;
#[doc(inline)]
pub use xcp::XcpError;
#[doc(hidden)] // For macro use only
pub use xcp::XcpEvent;
#[doc(inline)]
pub use xcp::XcpTransportLayer;
#[doc(hidden)] // For macro use only
pub use xcp::daq::daq_event::DaqEvent;

// @@@@ Reexport xcplib for xcplib_demo
pub use xcp::xcplib::*;

// Public submodule registry
pub mod registry;
pub use registry::McValueTypeTrait;

// Public submodule metrics
pub mod metrics;

// Used by macros
#[doc(hidden)]
pub use xcp_idl_generator::prelude::*;
#[doc(hidden)]
pub use xcp_type_description::prelude::*;
