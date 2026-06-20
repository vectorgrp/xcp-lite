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
pub use xcp::CalCell;
pub use xcp::CalSeg;
pub use xcp::Xcp;
pub use xcp::XcpEvent;
pub use xcp::XcpTransportLayer;
pub use xcp::daq::daq_event::DaqEvent;

// Public submodule metrics
pub mod metrics;

// Re-export the standalone registry crate under the old module path.
pub use xcp_registry as registry;
pub use registry::McValueTypeTrait;

// Used by macros
#[doc(hidden)]
pub use xcp_idl_generator::prelude::*;
#[doc(hidden)]
pub use xcp_type_description::prelude::*;

// EPK calibration segment
pub(crate) const EPK_SEG_NAME: &str = "epk";
pub(crate) const EPK_SEG_SIZE: usize = 31;
pub(crate) const EPK_SEG_ADDR: u32 = 0x80000000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
struct EpkSeg {
    epk: [u8; EPK_SEG_SIZE],
}
const EPK: EpkSeg = EpkSeg { epk: [0; EPK_SEG_SIZE] };
