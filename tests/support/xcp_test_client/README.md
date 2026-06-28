# xcp_test_client

Slim XCP-on-Ethernet client used **only** as the integration-test client for the
`xcp_lite` workspace. It is a library crate (no binary, no CLI) and is pulled in
as a `dev-dependency` by the workspace integration tests in [`tests/`](../../).

It provides just enough of the XCP protocol to drive the tests:

- Connect to an XCP-on-Ethernet server via TCP or UDP
- Upload the A2L from the server and load it into an `xcp_registry`
- Read and write calibration variables (CAL)
- Configure and acquire measurement data (DAQ)

## Usage

```rust
use xcp_test_client::*;

let mut client = XcpClient::new(/* ... */);
client.connect(/* ... */).await?;
client.upload_a2l_into_registry(/* ... */).await?;
```

## Relationship to the standalone tool

The full standalone tool — with the ELF/DWARF reader, Intel-HEX/BIN calibration
file I/O, and the command-line interface — lives in the `xcplib` submodule at
`xcplib/tools/xcpclient`. This crate is a trimmed-down copy that contains only
the protocol surface the workspace tests exercise.
