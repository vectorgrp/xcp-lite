
# XCPlite V1.0.0

XCPlite is a lightweight pure C implementation of the ASAM XCP V1.4 standard protocol for measurement and calibration of electronic control units.  
It supports XCP on TCP or UDP with jumboframes.  
Runs on 64 Bit platforms with POSIX based (LINUX, MACOS) or Windows Operating Systems.  
The A2L measurement and calibration object database is generated during runtime and uploaded by the XCP client on connect.

XCPlite is provided to test and demonstrate calibration tools such as CANape or any other XCP client implementation.  
It may serve as a base for individually customized XCP implementations on Microprocessors.  

XCPlite is used as a C library for the implementation of XCP for rust in:  
<https://github.com/vectorgrp/xcp-lite>  

New to XCP?  
Checkout the Vector XCP Book:  
<https://www.vector.com/int/en/know-how/protocols/xcp-measurement-and-calibration-protocol/xcp-book#>  

Visit the Virtual VectorAcedemy for an E-Learning on XCP:  
<https://elearning.vector.com/>  

## Whats new in V1.0.0

- Breaking changes to V6.  
- Lockless transmit queue. Works on x86-64 strong and ARM-64 weak memory model.  
- Measurement of and write access to variables on stack.  
- Supports multiple calibration segments with working and reference page with independent page switching
- Lock free and thread safe calibration parameter access, consistent calibration changes and page switches.  
- Build as a library.  
- Used (as FFI library) for the rust xcp-lite version.  

## Features

- Supports XCP on TCP or UDP with jumbo frames.  
- Thread safe, minimal thread lock and single copy event driven, timestamped high performance and consistent data acquisition.  
- Runtime A2L database file generation and upload.  
- Prepared for PTP synchronized timestamps.  
- Supports calibration and measurement of structures
- User friendly code instrumentation to create calibration parameter segments, measurement variables and A2L metadata descriptions.  
- Measurement of global or local stack variables.  
- Thread safe, lock-free and wait-free ECU access to calibration data.  
- Calibration page switching and consistent calibration.  

A list of restrictions compared to Vectors free XCPbasic or commercial XCPprof may be found in the source file xcpLite.c.  
XCPbasic is an optimized implementation for smaller Microcontrollers and with CAN as Transport-Layer.
XCPprof is a product in Vectors AUTOSAR MICROSAR and CANbedded product portfolio.  

## Examples  

hello_xcp:  
  Getting started with a simple demo in C with minimum code and features.  
  Shows the basics how to integrate XCP in existing applications.  

c_demo:  
  Shows more complex data objects (structs, arrays), calibration objects (axis, maps and curves).  
  Consistent calibration changes and measurement.  
  Calibration page switching and EPK version check.  
  