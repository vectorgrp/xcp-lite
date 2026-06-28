# xcp-lite

XCP for Rust - Rust API for XCPlite (<https://github.com/vectorgrp/XCPlite>)  
  

xcp-lite is a Rust API for measurement and calibration, which uses the ASAM XCP protocol for communication with a measurement and calibration tool like CANape and ASAM A2L for data description.  

This is no complete implementation of XCP in Rust, the protocol and transport layer implementation is in C/C++ based on XCPlite.  
For more details on XCP and XCPlite, see <https://github.com/vectorgrp/XCPlite>. The Rust API provides a convenient and safe interface to the C/C++ implementation, which is optimized for speed, lock-less operation and low memory footprint. The C/C++ XCPlite is a submodule of this repository. 

The Rust implementation provides its own in memory registry for measurement and calibration objects and types, which is used to generate the A2L file on target. It does not use the XCPlite A2L generation. The registry library crate is used by other crates to deal with A2L end ELF files.  

 
Main purpose was to experiment with Rust and to demonstrate some more advanced features of measurement and calibration with CANape:

- Automatic A2L and IDL generation with proc-macros
- A transparent Rust wrapper type for calibration variables which provides synchronized and memory safe calibration access
- Support for offline calibration, calibration page switching, reinit, load and save to json file
- Measurement of dynamic variables from stack or heap
- Measurement of variables with non static lifetime
- Measurement of thread local data instances
- Data objects and containers with dynamic size like point clouds or detection lists, to demonstrate CANape ADAS features
- Support Google protobuf or OMG DDS/CDR serialized data objects with XCP and CANape

Requires CANape 22 or later.  
  

## Examples  

The crate ships with a set of runnable examples under [examples/](examples/README.md), each paired
with a CANape project. See the [examples overview](examples/README.md) for the full list and the
build, run and command line instructions common to all of them.


## Build and Test

See the [examples overview](examples/README.md) for how to build and run the examples and the
command line options common to all of them.

### Features

- `a2l_reader` — parse and check the generated A2L file before upload.

### Build

```
cargo build
cargo build --release
cargo build --features a2l_reader
```

### Test

Tests must not run in parallel (the XCP implementation is a singleton), and the `a2l_reader`
feature is required for the `xcp_client` based tests:

```
cargo test --features a2l_reader -- --test-threads=1 --nocapture
cargo test --features a2l_reader -- --test-threads=1 --nocapture --test test_multi_thread
```

Use `--nocapture` because the debug output from the XCPlite C library is via plain printf.

## Notes

Like in C/C++ XCPlite, all measurement and calibration code instrumentation is non blocking and lock-free.
There are no heap allocation during runtime, except for the lazy registrations for A2L generation.
  
build.rs automatically builds a minimum static C library from individually pre configured core XCPlite sources.

The generated A2L file is finalized on XCP connect and provided for upload via XCP.

The proc macro for more convenient A2L generation is still in an experimental state.

Measurement of local variables is done with a macro which either copies to a static transfer buffer in the event or directly accesses the value on stack.  
This involves a lazy initialization of the structures to build the A2l file describing the local variables.  

The EPK version string in the A2L file can be set by the application. It resides a separate, hardcoded const memory segment.  

