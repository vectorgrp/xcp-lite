# xcp-lite V0.3.0

XCP for Rust - based on XCPlite  
  
xcp-lite is a Rust API for measurement and calibration, which uses the ASAM XCP protocol for communication with a measurement and calibration tool like CANape and ASAM A2L for data description.  

This is no complete implementation of XCP in Rust, the protocol and transport layer implementation is in C/C++ based on XCPlite.  

Main purpose was to experiment with Rust and to demonstrate some more advanced features of measurement and calibration with CANape:

- Automatic A2L and IDL generation with proc-macros
- A transparent wrapper for calibration variables which provides consistent and memory safe access to calibration parameters
- Support for offline calibration, calibration page switching, reinit, load and save to json file
- Measurement of dynamic variables from stack or heap
- Measurement of variables with non static lifetime
- Measurement of thread local data instances
- Data objects and containers with dynamic size like point clouds or detection lists, to demonstrate CANape ADAS features
- Support Google protobuf or OMG DDS/CDR serialized data objects with XCP and CANape

Requires CANape 23.
  
## Version History

Disclaimer: This code is in experimental state.  
There is no release tagging and semantic versioning yet.  

### V0.3.0

Registry data model and user API refactored  
Bugfixes

## Introduction

XCP is a measurement and calibration protocol commonly used in the automotive industry. It is an ASAM standard.  

It provides time series signal data acquisition (measurement) and modification of parameter constants (calibrations) in a target micro controller system (ECU), to help observing and optimizing control algorithms in real time.  
  
Timestamped events, measurement variables and parameter constants are described by an ASAM-A2L description file, another associated ASAM standard.
Data objects are identified by an address. In a micro controller system written in C or C++, these addresses are used to directly access the ECUs memory, like a debugger would do. This concept has minimum impact on the target system in terms of memory consumption and runtime. The A2l is a kind of annotated ELF Linker-Address-Map, with rich semantic information on data instances and data types.  
In a higher abstraction level programming language, XCP can be treated as a serializer/deserializer, where A2L is the schema, which is generated from the target software data types and instances. Measurement signals and calibration parameters must have static lifetime and a defined memory layout, but no predefined memory location. Data acquisition and modification is achieved by appropriate code instrumentation for measurement and wrapper types for calibration parameters and parameter groups.  

The ASAM-XCP standard defines a protocol and a transport layer. There are transport layers for all common communication busses used in the automotive industry, such as CAN, CAN-FD, FLEXRAY, SPI and Ethernet.  

XCPlite (<https://github.com/vectorgrp/XCPlite>) is a simplified implementation of XCP in C,C++, optimized for the XCP on Ethernet Transport Layer.  

In C or C++ software, A2L data objects are usually created with global or static variables, which means they have a constant memory address. XCPlite for C++ introduced an additional code instrumentation concept to measure and calibrate instances of classes located on heap. It is still using direct memory access, but A2L addresses are relative and the lifetime of measurement variables is associated to events.

An implementation of XCP in Rust, with direct memory access, will get into conflict with the memory and concurrency safety concepts of Rust. In Rust, mutating static variables by using pointers is considered Unsafe code, which might create undefined behavior in parallel access. Thread safety when accessing any data will be strictly enforced.

xcp-lite (<https://github.com/vectorgrp/xcp-lite>) is an implementation of XCP for Rust. It provides a user friendly concept to wrap structs with calibration parameters in a convenient and thread safe type, to make calibration parameters accessible and safely interior mutable by the XCP client tool.
To achieve this, the generation of the A2L description is part of the solution. In XCPlite this was an option.
A2L objects for events and measurement values will be lazily created during startup, using a runtime registry and a proc-macro to create calibration parameter descriptions from structs.

The calibration parameter wrapper type CalSeg enables all advanced calibration features of a measurement and calibration tool like CANape. An instance of CalSeg creates a memory segment, which enables version checking, checksum calculation, offline and indirect calibration, page switching and parameter persistence (freeze and init). It also provides parameter persistence to a json file.  

xcp-lite also implements a concept to measure variables on stack or in thread local memory.

Currently xcp-lite for Rust uses a C library build from XCPlite sources, which contains the XCP server, an ethernet transport layer with its rx/tx server threads, the protocol layer, time stamp generation and time synchronization. The C implementation is optimized for speed by minimizing copying and locking data. There are no heap allocations. The Rust layer includes the registry and A2L generation, wrapper types for calibration parameters and macros to capture measurement data on events.

The code should work on Linux, Windows and Mac, Intel and ARM.  
  
The project creates a library crate xcp_lite and a main application to demonstrate all use case. A entry level example is hello_xcp in the example folder. There are other, more specific examples in the examples folder.  
There is an integration test, where the crate a2lfile is used to verify the generated A2L file and a limited, tokio based XCP client with DAQ decoding for black box testing.

## Examples  

### xcp-lite (xcp-lite/src/main.rs)

Main application of the crate
Does not serve demonstration purposes, better refer to the examples below
Manually check various measurement and calibration features with the CANape project in ./CANape  

### hello_xcp

A very basic example  
Measure a local variable and calibrate a parameter of basic scalar type

### single_thread_demo

Demonstrates how to measure and calibrate variables in a thread
Shows how to clone and move a calibration parameter set in a CalSeg to a thread  
Shows how to load and save calibration parameters from json files

### multi_thread_demo

Demonstrates how to measure and calibrate a task instantiated in multiple threads with multiple instances of measurement events and local variables  
Shows how to share calibration parameters among thread using a static CalCell

### struct_measurement_demo

Demonstrates measurement data collection of more complex types, such as struct, arrays of struct and multi-dimensional array slices
This demo generates A2L objects TYPEDEF and INSTANCES  
It also contains an example how to use the experimental histogram metric type

### calibration_demo

Demonstrates various calibratable basic types, nested structs and multi dimensional types with shared axis and associated lookup functions with interpolation  
This demo generate A2L objects CURVE and MAP with shared AXIS_PTS  

### rayon_demo

Use CANape to observe rayon workers calculating a mandelbrot set line by line

### tokio_demo

Demonstrates using XCP in an async tokio based application

### point_cloud_demo

Measure a lidar point cloud and visualize it in CANapes 3D scene window  
Use CDR serialization over XCP and the CDR/IDL schema generator proc-macro

## Code instrumentation for measurement and calibration
  
There are 3 important types: Xcp, XcpEvent/DaqEvent and CalSeg.  
Xcp is a wrapper for XCPlite. It is a singleton.
  
CalSeg is a generic type used to encapsulate structs containing calibration parameters. This is called a calibration segment and the parameter struct wrapped is a calibration page. A calibration page must be Copy and may contain nested structs of basic types or arrays with dimension up to 2.  
  
A CalSeg has interior mutability. Parameters may be accessed in a safe way with RAII guards.
  
A CalSeg may be shared among multiple threads. It is send and can be cloned like like a smart pointer, such as Arc. There is a shared mutex for XCP write access among all clones. Each clone holds a shadow copy of the calibration values on heap.

To provide global access to a calibration segment, CalCell is provided. CalCell holds a calibration segment. It is sync. Calibration segment clone may be obtained from a CalCell.

Measurement code instrumentation provides event definition, registration or capture of measurement objects. Measurement objects can be captured (copied to a buffer inside the event) or accessed directly on stack memory after being registered. Capture works for variables on heap or stack. Measurement variables can be registered as single instance or multi instance, which creates one variable instance for each thread instance. Variable names and event names are automatically extended with an index in this case.

The registration of objects has to be completed, before the A2L file is generated.
The A2L is created at latest on connect of the XCP client tool. Objects created later, will not be visible to the tool.  
  
``` rust

// Calibration parameter segment
// Each calibration parameter struct defines a MEMORY_SEGMENT in A2L and CANape
// The A2L serializer will create an A2L CHARACTERISTIC for each field. 
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage {

    #[charateristic(comment = "Amplitude")]
    #[characteristic(unit = "Volt")]
    #[characteristic(min = "0")]
    #[characteristic(max = "400")]
    ampl: f64,

    #[characteristic(comment = "Period")]
    #[characteristic(unit = "s")]
    #[characteristic(min = "0")]
    #[characteristic(max = "1000")]
    period: f64,
}

// Default calibration page values (called "FLASH" page of a MEMORY_SEGMENT in CANape)
const CAL_PAGE: CalPage = CalPage {
    ampl: 100.0,
    period: 5.0,
};


// A single instance demo task 
// Calculates some measurement signals depending on calibration parameters in a calibration segment
fn task(calseg: CalSeg<CalPage>) {

    let mut channel1: f64 = 0.0;
    let mut channel2: f64 = Box::new(0.0);

     // Create a measurement event called "task" with a capture buffer of 8 byte
    let event = daq_create_event!("task1", 8);

    // Register measurement variables on stack
    daq_register!(channel, event, "demo: f64", "Volt" /* unit */, 2.0 /* factor */, 0.0 /* offset */);

    loop {
        thread::sleep(...);

        // Synchronize calibration operations in calseg
        // All calibration actions (read, write, upload, download, checksum, page switch, freeze, init) on segment "cal_seg" happen only here
        // This operation locks a mutex, checks for changes and copies the calibration page
        // Calibration parameters are consistent, while the guard exists
        {
          let calseg = calseg.read_lock();

          // Calculate channel depending on calibration parameters from calseg (sine wave signal with ampl and period)
          channel1 = calseg.ampl * (time/cal_seg.period).sin(); // Use active page in calibration segment
          channel2 = calseg.ampl * (time/cal_seg.period).sin(); // Use active page in calibration segment
        }
        
        // Register and capture a variables on heap 
        daq_capture(channel1);

        // Take a timestamp and trigger data acquisition for all variables associated and configured by the tool for this event
        event.trigger(); 

      
    }
}



fn main() -> Result<()> {

    // Initialize XCP driver singleton, the transport layer UDP and enable the automatic A2L writer and upload
    let xcp = XcpBuilder::new("my_module_name").set_log_level(2).set_epk("MY_EPK")
      .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1024*64)?;

    // Create a calibration parameter set named "calseg" (struct CalSeg, a MEMORY_SEGMENT in A2L and CANape)
    // Calibration segments have 2 pages, a constant default "FLASH" reference page (CAL_PAGE) and a mutable "RAM" working page
     let calseg = xcp.create_calseg(
        "calseg", // name of the calibration segment in A2L as MEMORY_SEGMENT and as .json file
        &CAL_PAGE, // default calibration values
        ).register_fields();

    // Use CalSeg::Clone() to share the calibration segments between threads
    // No locks, calseg.sync() must be called in each thread
    thread::spawn({
        let calseg = CalSeg::clone(&calseg);
        move || {
            task(calseg);
        }
    });


    loop { ... }

    Xcp::stop_server();
}


```

## Safety Considerations

The fundamental functional concept of this XCP implementation is, to mutate the calibration variables in their original binary representation in a thread safe, transparent wrapper type.  
The implementation restricts memory accesses to the inner calibration page of a calibration segment, but does not check the correctness of modifications inside the calibration page.
As usual, the invariants to consider this safe, include the correctness of the A2L file and of the XCP client tool. When the A2L file is uploaded by the XCP tool on changes, this is always guaranteed.
The wrapper type is Send, not Sync and implements a RAII guard pattern to access parameters.

Code in Unsafe blocks exists in the following places:

- The implementation of Sync for CalCell.
- The XCPlite bindings XcpEventExt for measurement and cb_read/cb_write for calibration, which carry byte pointers and memory offsets of measurement and calibration objects  
- Synchronisation of the shared XCP calibration page with the working page of each CalSeg clone
- And formally all calls to the C FFI of the XCPlite server (optional), transport layer and protocol layer  

A measurement and calibration concept without any code in unsafe blocks is practically impossible to achieve, without massive consequences for the API, which would lead to much more additional boilerplate code to achieve calibration.
The memory oriented measurement and calibration approach of XCP is very common in the automotive industry and there are many tools, HIL systems and loggers supporting it.  
XCP is used during the development process only, it is never integrated in production code or it is disabled by qualified code.

## Build

### Features

Features are:

- a2l_reader
Check A2L file after generation before upload

### Build, Run, Test

Build, Run, Test examples:

```

Build:
  cargo b 
  cargo b --release 
  cargo b --features a2l_reader

Run the main example:
  cargo r -- --bind 127.0.0.1 --log-level 4
  cargo r -- --port 5555 --bind 172.19.11.24 --tcp 
 
Run a specific example:
  cargo r --example struct_measurement_demo
  cargo r --example xcp_client  

```

Test

Tests may not run in parallel, as the XCP implementation is a singleton.
Feature a2l_reader is needed for xcp_client based testing

```

  cargo test --features=a2l_reader  -- --test-threads=1 --nocapture
  cargo test --features=a2l_reader  -- --test-threads=1 --nocapture  --test test_multi_thread
 
```

Use --nocapture because the debug output from the XCPlite C library is via normal printf

```


## Notes

All measurement and calibration code instrumentation is non blocking and the trigger event and sync methods is optimized for speed and minimal locking.  
There are no heap allocation during runtime, except for the lazy registrations of and for A2L generation, or when cloning a CalSeg.
  
build.rs automatically builds a minimum static C library from individually pre configured core XCPlite sources.
On C level, there is a synchronization mutex for the mpsc transmit queue.  
The C code has the option to start the server with 2 normal threads for rx and tx socket handling.

The generated A2L file is finalized on XCP connect and provided for upload via XCP.

The proc macro for more convenient A2L generation is still in an experimental state.

Measurement of local variables is done with a macro which either copies to a static transfer buffer in the event or directly accesses the value on stack.  
This involves a lazy initialization of the structures to build the A2l file describing the local variables.  

There are 4 different addressing schemes, indicated by address extension (called _ABS, _REL,_DYN and _APP in the code).  
In mode APP, the low word of a calibration parameters memory address in the A2L file is a relative offset in the calibration page struct.  
The high word (& 0x7FFF) is the index of the calibration segment in a alphabetic ordered list.  
The memory addresses of local measurement variables are relative addresses (mode DYN) in their event capture buffer on stack or to the stack location of the variable holding the event.
Mode ABS is the usual absolute addressing mode, relative to the module load address, which is only useful for static cells.  
This is not used for 64 bit systems, as A2L only supports 32 bit addresses.  
These concepts are currently not supported by the A2L update tools, though A2L generation at runtime is the only option for now.  

The EPK version string in the A2L file can be set by the application. It resides a separate, hardcoded const memory segment.  

## CANape

To use one of the CANape projects included, use 'Project/Open" and select the file CANape.ini in the CANape folder.  

Please check the IP address in the CANape device manager.
Because the A2L file is uploaded by CANape, the IP address must be known in advance.  

The examples are build with CANape 23.
Older versions were not tested.

![CANape](CANape.png)
