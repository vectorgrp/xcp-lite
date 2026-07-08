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
pub use registry::McValueTypeTrait;
pub use xcp_registry as registry;

// Re-export the McRegisterType trait and its derive macro (one import brings both).
pub use registry::McRegisterType;
// Re-export the McRegisterEnum derive (for integer enums used in registered structs).
pub use registry::McRegisterEnum;

// Used by macros
#[doc(hidden)]
pub use xcp_idl_generator::prelude::*;

// Internal re-exports used by the cal_seg! macro. Not part of the public API.
#[cfg(feature = "linkme")]
#[doc(hidden)]
pub mod _private {
    pub use crate::xcp::{CAL_SEG_REGISTRY, CalSegDescriptor};
    pub use linkme::distributed_slice;
}

// EPK calibration segment definitions, must match libxcplite definitions
pub(crate) const EPK_SEG_NAME: &str = "epk";
pub(crate) const EPK_SEG_SIZE: usize = 31;
pub(crate) const EPK_SEG_ADDR: u32 = 0x80000000;

// @@@@ TODO: Remove
// #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
// struct EpkSeg {
//     epk: [u8; EPK_SEG_SIZE],
// }
// const EPK: EpkSeg = EpkSeg { epk: [0; EPK_SEG_SIZE] };
