# Changelog

All notable changes to Rust xcp-lite are documented in this file.


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

  

