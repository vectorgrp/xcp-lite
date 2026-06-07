//-----------------------------------------------------------------------------
// Library crate xcpclient
// xcpclient is a library crate that provides an XCP on ETH client implementation for integration testing

// This crate is a library
#![crate_type = "lib"]
// The library crate is named "xcp_client"
#![crate_name = "xcp_client"]

pub mod bin_reader;
pub mod elf_reader;
pub mod xcp_client;
