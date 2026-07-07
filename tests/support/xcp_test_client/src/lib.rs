//-----------------------------------------------------------------------------
// Library crate xcp_test_client
// Slim XCP-on-ETH client used only as the integration-test client for the
// xcp_lite workspace (see ../../../tests). The full standalone tool (with the
// ELF/DWARF reader and CLI) lives in the xcplib submodule: xcplib/tools/xcpclient.

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp_test_client"
#![crate_name = "xcp_test_client"]

mod xcp_client;
pub use xcp_client::*;
