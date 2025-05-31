
# XCPlite V1.x.x

XCPlite is a lightweight pure C implementation of the ASAM XCP V1.4 standard protocol for measurement and calibration of electronic control units.
It supports XCP on UDP or TCP.
It runs on POSIX based (LINUX,MACOS) or Windows Operating Systems.  

XCPlite is provided to test and demonstrate calibration tools such as CANape or any other XCP client implementation.  
It demonstrates some capabilities of XCP and may serve as a base for individually customized implementations.  

XCPlite is used as C library for the Rust implementation of XCP:  
<https://github.com/vectorgrp/xcp-lite>

New to XCP?  
Checkout the Vector XCP Book:  
<https://www.vector.com/int/en/know-how/protocols/xcp-measurement-and-calibration-protocol/xcp-book#>

Visit the Virtual VectorAcedemy for an E-Learning on XCP:  
<https://elearning.vector.com/>

## Features

- Supports XCP on TCP or UDP with jumbo frames.
- Thread safe, minimal thread lock and single copy event driven, timestamped data acquisition.  
- Runtime A2L file generation and upload.  
- Measurement of global or local stack variables.
- Thread safe, lock-free and wait-free access to calibration data.  
- Supports calibration page switching.  
- Can be build as a library.  
- It is used as the XCP on ETH protocol and transport layer implementation for the rust xcp-lite API

A list of restrictions compared to Vectors free XCPbasic or commercial XCPprof may be found in the source file xcpLite.c.  
XCPbasic is an implementation optimized for smaller Microcontrollers and CAN as Transport-Layer.  
XCPprof is a product in Vectors AUTOSAR MICROSAR and CANbedded product portfolio.

## Included code examples  

hello_xcp:
  Getting started with a simple demo in C with minimum code and features.
  Shows the basics how to integrate XCP in existing applications.
  
c_demo:
  Shows more sophisticated calibration, maps and curves, calibration page switching and EPK check.
  