# Changelog

All notable changes to Rust xcp-lite are documented in this file.


## [V3.0.10]

- A2L reader: IF_DATA XCP is always parsed with the XCP 1.4 A2ML specification built into `xcp_registry`; A2L files no longer need to include or contain an A2ML description to be loaded or checked. An A2L file with `/include "XCP_104.aml"` still requires the file next to it.
- A2L reader: the built-in A2ML specification (`aml_ifdata.rs`) is updated to XCP 1.4 (XCP_104.aml) and CANape (CANape.aml): `PACKET_ALIGNMENT` and `OPTIONAL_TL_SUBCMD` of all transport layers, `CAN_ID_GET_DAQ_CLOCK_MULTICAST`, `MEASUREMENT_SPLIT_ALLOWED`, `BUFFER_RESERVE_EVENT`, `CORE_LOAD_EP`, `CPU_LOAD_CONSUMPTION_QUEUE_STIM`, FlexRay `MULTICAST`, CANape `STEPSIZE_W/X/Y` and `CNP_CREATE_INI`. Before, an IF_DATA XCP using any of these (e.g. `OPTIONAL_TL_SUBCMD GET_DAQ_CLOCK_MULTICAST` written by XCPlite with multicast enabled) could not be decoded and its events were missing. Unit tests check that the built-in specification parses IF_DATA identically to XCP_104.aml and CANape.aml.
- A2L reader: non-XCP or unparsable IF_DATA blocks are skipped instead of panicking, a module IF_DATA which can not be decoded with the built-in specification is reported with a warning.
- A2L writer: `Registry::set_a2ml_mode(McA2mlMode)` selects how the A2ML description is written: `Include` (`/include "XCP_104.aml"`, default, unchanged), `Embed` (self-contained A2L file with `/begin A2ML ... /end A2ML`) or `None`.
- `xcp_registry::XCP_104_AML` provides the XCP 1.4 A2ML description (XCP_104.aml) as a string.


## [V3.0.9]

- Bugfix: `McDimType::is_scalar`/`is_array`/`is_matrix` — copy-paste bug caused misclassifying all 1D arrays as scalars.
- Bugfix: EPK string sanitization (spaces, tabs, colons → `_`) is now applied in the xcp-lite registry to match XCPlite behavior; `Xcp::init()` pre-sanitizes before passing to both `XcpInit()` and the registry.
- `McRegisterType` derive macro now handles `syn::Type::Group` wrapping via `unwrap_type_group`, fixing type parsing inside `macro_rules!` contexts (issue #51). Regression test added.
- Added `McXcpProtocolLayer` (`max_cto`, `max_dto`) to the registry; written into A2L XCP IF_DATA.
- A2L files are now written with a UTF-8 BOM.
- A2L writer: unique names for field typedefs of the same name but different type.
- Removed the special treatment of event id 0 as a default event instead of a fixed event
- Switched xcplib submodule to VectorGrp master.


## [V3.0.8]

- Explicit `read_write` qualifier added to measurement (`#[measurement(qualifier = "read_write", ...)]`) fields.
- More idiomatic iterator implementations for `McInstanceList`, `McTypedefList`, `McEventList`, and `McCalibrationSegmentList`.
- Switched xcplib submodule to V2.1.9.


## [V3.0.5]

- Fix xcplib compilation on Linux/GCC with -std=c11
    Added -D_GNU_SOURCE to cc build flags (build.rs)
    GCC on Linux with -std=c11 disables all POSIX/GNU extensions unless explicitly opted in. 
    The following symbols in platform.c require _GNU_SOURCE to be visible:
        - CLOCK_MONOTONIC_RAW, CLOCK_REALTIME  (GNU/Linux clock IDs)
        - clock_gettime, clock_getres          (POSIX.1b realtime clock)
        - nanosleep                            (POSIX.1b sleep)
        - pthread_mutexattr_settype, PTHREAD_MUTEX_RECURSIVE (POSIX.1-2001 mutex attrs)
        - struct timeval, struct ip_mreqn      (socket/network types)
        - gmtime_r                             (POSIX.1-2001 time util)
- Fix A2L writer: `XCP_ON_SXI` IF_DATA block now includes explicit `FRAMING` to enable SYNC mode in CANape.


## [V3.0.3]

- Removed some clippy warnings


## [V3.0.1]

- Support for enum fields of basic types (u8, u16, u32, i8, i16, i32) in the new McRegisterType macro.  
- Deterministic calibration segment registration via the new `linkme` feature (enabled by default).  
- Flattening supports multi-dimensional types using the usual A2L mangling syntax.  
- Moved the xcp-lite binary to examples/all_feature_demo.  
- Switched the XCPlite library configuration to the new XCPlite configuration override concept.  


## [V3.0.0]

- New McRegisterType and McRegisterEnum derive macros replace the old xcp_type_description_derive macro. The new macros supports multi-dimensional types and scalar enums, it has more comprehensive syntax and improved error messages at compile time instead of panicking. All examples have been migrated to the new macros.  

- Separate registry crate.  
- xcpclient tool moved to XCPlite repo, because it is required for RTOS use cases. Specialized xcpclient lib kept for integration tests. 
- Sync with XCPlite changes.  
- Bug-Fixes.  

  

