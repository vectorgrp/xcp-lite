//-----------------------------------------------------------------------------
// Library crate xcp_client
// Slim XCP-on-ETH client used only as the integration-test client for the
// xcp_lite workspace (see ../../tests). The full standalone tool (with the
// ELF/DWARF reader and CLI) lives in the xcplib submodule: xcplib/tools/xcpclient.

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp_client"
#![crate_name = "xcp_client"]

pub mod bin_reader;
pub mod xcp_client;
