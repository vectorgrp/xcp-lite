//-----------------------------------------------------------------------------
// Application xcp
// XCPlite demo for Rust, crate xcp
// (c) 2024 by Vector Informatik GmbH
//
// Demonstrates the usage of xcp-lite for Rust together with a CANape project

#![allow(dead_code)] // Demo code
#![allow(clippy::vec_init_then_push)]
#![allow(unused_imports)]

use std::{
    f64::consts::PI,
    fmt::Debug,
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

//-----------------------------------------------------------------------------

const TASK1_CYCLE_TIME: u32 = 10000; // 10ms
const TASK2_CYCLE_TIME: u32 = 10000; // 10ms
const TASK2_INSTANCE_COUNT: usize = 10;
const MAINLOOP_CYCLE_TIME: u32 = 100; // 100ms

//-----------------------------------------------------------------------------
// Logging

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

//-----------------------------------------------------------------------------
// Command line arguments

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = 3)]
    log_level: u8,

    /// Bind address
    #[arg(short, long, default_value_t = Ipv4Addr::new(127, 0, 0, 1))]
    bind: Ipv4Addr,

    /// Use TCP as transport layer, default is UDP
    #[arg(short, long, default_value_t = false)]
    tcp: bool,

    /// Port number
    #[arg(short, long, default_value_t = 5555)]
    port: u16,
}

//-----------------------------------------------------------------------------
// XCP

use xcp::*;
#[cfg(feature = "auto_reg")]
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Static variables

lazy_static::lazy_static! {

    // Application start time
    static ref START_TIME: Instant = Instant::now();

    // Stop all tasks if false
    static ref RUN: AtomicBool = AtomicBool::new(true);
}

struct StaticVars {
    test_u32: u32,
    test_f64: f32,
}

// Statically allocate memory for a `u32`.
static STATIC_VARS: static_cell::StaticCell<StaticVars> = static_cell::StaticCell::new();

//-----------------------------------------------------------------------------
// Static calibration data example
// This is the classical address oriented calibration approach for indivual calibration parameters or structs
// The calibration parameters are defined as static instances with constant memory address
// Each variable or struct field has to be registered manually in the A2L registry
// A2L addresses are absolute in the application process memory space (which means relative to the module load address)

// This approach uses a OnceCell to initialize a static instance of calibration data, a mutable static instead would need unsafe, a static might be in write protected memory and a const has no memory address
// The inner UnsafeCell allows interiour mutability, but this could theoretically cause undefined behaviour or inconsistencies depending on the nature of the platform
// Many C,C++ implementations of XCP do not care about this, but this approach is not recommended for rust projects

struct CalPage00 {
    task1_cycle_time_us: u32, // Cycle time of task1 in microseconds
    task2_cycle_time_us: u32, // Cycle time of task2 in microseconds
}

static CAL_PAGE0: once_cell::sync::OnceCell<CalPage00> = once_cell::sync::OnceCell::with_value(CalPage00 {
    task1_cycle_time_us: TASK1_CYCLE_TIME,
    task2_cycle_time_us: TASK2_CYCLE_TIME,
});

//-----------------------------------------------------------------------------
// Dynamic calibration data example
// This approach uses the segment oriented calibration approach with a calibrastion segment wrapper cell type
// It provides defined behaviour, thread safety and data consistency
// Fields may be automatically added to the A2L registry by the auto_reg feature and the XcpTypeDescription derive macro
// Each page defines a MEMORY_SEGMENT in A2L and CANape
// A2l addresses are relative to the segment start address, the segment numer is coded in the address

// Implement Serialize, Deserialize for json file persistency
// Implement XcpTypeDescription for auto registration of fields in the A2L registry

//---------------------------------------------------
// CalPage
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "auto_reg", derive(XcpTypeDescription))]
#[derive(Debug, Clone, Copy)]
struct CalPage {
    run: bool,          // Stop all tasks
    run1: bool,         // Stop demo task1
    run2: bool,         // Stop demo task2
    cycle_time_ms: u32, // Cycle time of main loop task in milliseconds
}

const CAL_PAGE: CalPage = CalPage {
    run: true,
    run1: true,
    run2: true,
    cycle_time_ms: MAINLOOP_CYCLE_TIME,
};

//---------------------------------------------------
// CalPage1
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "auto_reg", derive(XcpTypeDescription))]
#[derive(Debug, Clone, Copy)]
struct TestInts {
    test_bool: bool,
    test_u8: u8,
    test_u16: u16,
    test_u32: u32,
    test_u64: u64,
    test_i8: i8,
    test_i16: i16,
    test_i32: i32,
    test_i64: i64,
    test_f32: f32,
    test_f64: f64,
}

#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "auto_reg", derive(XcpTypeDescription))]
#[derive(Debug, Clone, Copy)]
struct CalPage1 {
    counter_max: u32,

    // Other basic types supported
    test_ints: TestInts,
}

const CAL_PAGE1: CalPage1 = CalPage1 {
    counter_max: 1000,

    test_ints: TestInts {
        test_bool: false,
        test_u8: 0x12,
        test_u16: 0x1234,
        test_u32: 0x12345678,
        test_u64: 0x0102030405060708u64,
        test_i8: -1,
        test_i16: -1,
        test_i32: -1,
        test_i64: -1,
        test_f32: 0.123456E-10,
        test_f64: 0.123456789E-100,
    },
};

//---------------------------------------------------
// CalPage2
#[cfg(feature = "auto_reg")]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "auto_reg", derive(XcpTypeDescription))]
#[derive(Debug, Clone, Copy)]
struct CalPage2 {
    #[type_description(comment = "Amplitude")]
    #[type_description(unit = "Volt")]
    #[type_description(min = "0")]
    #[type_description(max = "400")]
    ampl: f64, // This will be a VALUE type

    #[type_description(comment = "Period")]
    #[type_description(unit = "s")]
    #[type_description(min = "0")]
    #[type_description(max = "1000")]
    period: f64, // This will be a VALUE type

    #[type_description(comment = "Demo curve", unit = "ms", min = "0", max = "100")]
    array: [f64; 16], // This will be a CURVE type (1 dimension)

    #[type_description(comment = "Demo map", unit = "ms", min = "-100", max = "100")]
    map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)
}

#[cfg(not(feature = "auto_reg"))]
#[derive(Debug, Clone, Copy)]
struct CalPage2 {
    ampl: f64,         // Amplitude of the demo sine signals
    period: f64,       // Period of the demo sine signals
    array: [f64; 16],  // Demo curve
    map: [[u8; 9]; 8], // Demo map
}

const CAL_PAGE2: CalPage2 = CalPage2 {
    ampl: 100.0,
    period: 1.0,
    array: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    map: [
        [0, 0, 0, 0, 0, 0, 0, 1, 2],
        [0, 0, 0, 0, 0, 0, 0, 2, 3],
        [0, 0, 0, 0, 0, 1, 1, 2, 3],
        [0, 0, 0, 0, 1, 1, 2, 3, 4],
        [0, 0, 1, 1, 2, 3, 4, 5, 7],
        [0, 1, 1, 1, 2, 4, 6, 8, 9],
        [0, 1, 1, 2, 4, 5, 8, 9, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
    ],
};

//-----------------------------------------------------------------------------
// Demo application cyclic tasks in threads

// A task which calculates some measurement signals depending on calibration parameters in a shared calibration segment
// This task is instantiated multiple times
fn task2(task_id: usize, calseg: CalSeg<CalPage>, calseg2: CalSeg<CalPage2>) {
    // Static calibration parameters
    let calpage0 = CAL_PAGE0.get().unwrap();

    // Create an event instance for each thread, with 8 byte capture buffer
    let mut instance_event = daq_create_event_tli!("task2_inst", 8);

    // Create one static event for all instances of this thread, with 8 byte capture buffer
    let mut event = daq_create_event!("task2_static", 8);

    while RUN.load(Ordering::Acquire) {
        // Stop task if calibration parameter run2 is false
        if !calseg.run2 {
            break;
        }

        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(calpage0.task2_cycle_time_us as u64));

        // Calculate demo measurement variable depending on calibration parameters (sine signal with ampl and period)
        let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
        let offset = task_id as f64 * 10.0;
        let channel = offset + calseg2.ampl * (PI * time / calseg2.period).sin(); // Use active page in calibration segment

        // Measurement of local variables by capturing their value and association to the given XCP event

        // daq_capture_tli adds the event id to the signal name to make the instances of <channel> in different threads unique
        daq_capture_tli!(channel, instance_event, "sine: f64", "Volt");
        instance_event.trigger(); // Take a timestamp and trigger a multi instance data acquisition event

        // daq_capture creates a static signal for all instances of this thread
        daq_capture!(channel, event, "sine: f64", "Volt");
        event.trigger(); // Take a timestamp and trigger the static data acquisition event

        // Synchronize calibration operations
        // All calibration write operations (download, page switch, init) on a segment happen here
        calseg.sync();
        calseg2.sync();
    }
    info!("Task2 instance {} finished", task_id);
}

// A task with a single instance which calculates some counter signals of basic types and calibratable sawtooth counter
fn task1(calseg: CalSeg<CalPage>, calseg1: CalSeg<CalPage1>) {
    let mut counter: u32 = 0;
    let mut counter_u8: u8 = 0;
    let mut counter_u16: u16 = 0;
    let mut counter_u32: u32 = 0;
    let mut counter_u64: u64 = 0;
    let mut array1 = [0.0; 256];
    for (i, a) in array1.iter_mut().enumerate() {
        *a = i as f64;
    }

    // Static calibration parameters
    let calpage0 = CAL_PAGE0.get().unwrap();

    // Create an event with capture capacity of 1024 bytes for point_cloud serialization
    let event = daq_create_event!("task1");

    // Register signals of bassic types or array to be captured directly from stack
    daq_register!(counter, event, "", "", 1.0, 0.0);
    daq_register!(counter_u8, event, "wrapping counter: u8", "");
    daq_register!(counter_u16, event, "wrapping counter: u16", "");
    daq_register!(counter_u32, event, "wrapping counter: u32", "");
    daq_register!(counter_u64, event, "wrapping counter: u64", "");
    daq_register_array!(array1, event);

    while RUN.load(Ordering::Acquire) {
        // Stop task if calibration parameter run1 is false
        if !calseg.run1 {
            break;
        }

        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(calpage0.task1_cycle_time_us as u64));

        // Basic types and array variables on stack
        counter = counter.wrapping_add(1);
        if counter > calseg1.counter_max {
            counter = 0
        }
        counter_u8 = counter_u8.wrapping_add(1);
        counter_u16 = counter_u16.wrapping_add(1);
        counter_u32 = counter_u32.wrapping_add(1);
        counter_u64 = counter_u64.wrapping_add(1);
        array1[(counter_u16 % (array1.len() as u16)) as usize] = counter as f64;

        // Trigger single instance event "task1" for data acquisition
        // Capture variables from stack happens here
        event.trigger();

        // Sync the calibration segments
        calseg1.sync();
        calseg.sync();
    }
    info!("Task1 finished");
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() {
    println!("XCP for Rust - CANape Demo (project ./CANape)");

    let args = Args::parse();
    let log_level = XcpLogLevel::from(args.log_level);

    // Logging
    env_logger::Builder::new().filter_level(log_level.to_log_level_filter()).init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L writer
    let xcp = XcpBuilder::new("xcp_lite")
        .set_log_level(log_level)
        // .set_epk(build_info::format!("{}", $.timestamp)); // Create new EPK from build info
        .set_epk("EPK_")
        .start_server(if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp }, args.bind, args.port)
        .map_err(|e| {
            panic!("XCP server initialization failed: {:?}", e);
        })
        .unwrap();

    // Register a static calibration page
    let calpage00 = CAL_PAGE0.get().unwrap();
    cal_register_static!(calpage00.task1_cycle_time_us, "task1 cycle time", "us");
    cal_register_static!(calpage00.task2_cycle_time_us, "task2 cycle time", "us");

    // Create calibration parameter sets
    // Calibration segments have "static" lifetime, the Xcp singleton holds a smart pointer clone to each
    // When a calibration segment is dropped by the application and sync is no longer called, the XCP tool will get a timeout when attempting to access it
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze), reinitialized from default FLASH page (XCP copy_cal_page)
    // The initial RAM page can be loaded from a json file (load_json=true) or set to the default FLASH page (load_json=false)

    // Create a calibration segment wrapper for CAL_PAGE, add fields manually to registry
    let calseg = xcp.add_calseg(
        "CalPage", // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values with static lifetime, trait bound from CalPageTrait must be possible
    );
    calseg
        .add_field(calseg_field!(CAL_PAGE.run, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.run1, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.run2, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.cycle_time_ms, "ms", "main task cycle time"));

    // Create calibration segments for CAL_PAGE1 and CAL_PAGE2, add fields with macro derive(XcpTypeDescription))
    let calseg1 = xcp.create_calseg("CalPage1", &CAL_PAGE1, true);
    let calseg2 = xcp.create_calseg("CalPage2", &CAL_PAGE2, true);

    // Task2 - 9 instances
    // To demonstrate the difference between single instance and multi instance events and measurement values
    let mut t = Vec::with_capacity(TASK2_INSTANCE_COUNT);
    for i in 0..TASK2_INSTANCE_COUNT {
        let c1 = CalSeg::clone(&calseg);
        let c2 = CalSeg::clone(&calseg2);
        t.push(thread::spawn(move || {
            task2(i, c1, c2);
        }));
    }

    // Task1 - single instance
    // calseg1 moved, calseg cloned
    let c = CalSeg::clone(&calseg);
    let t1 = thread::spawn(move || {
        task1(c, calseg1);
    });

    // Mainloop
    xcp_println!("Main task starts");

    // Variables on heap and stack
    let mut mainloop_counter1: u64 = 0;
    let mut mainloop_counter2 = Box::new(0u64);
    let mut mainloop_map = Box::new([[0u8; 16]; 16]);

    let mut mainloop_event = daq_create_event!("mainloop", 64); // Capture buffer 64 bytes
    daq_register!(mainloop_counter1, mainloop_event);

    // Mutable static variables
    let static_event = xcp.create_event("static_event");
    let static_vars: &'static mut StaticVars = STATIC_VARS.init(StaticVars { test_u32: 0, test_f64: 0.0 });
    static_vars.test_u32 = 1;
    assert_eq!(static_vars.test_u32, 1);
    daq_register_static!(static_vars.test_u32, static_event, "Test static u32");
    daq_register_static!(static_vars.test_f64, static_event, "Test static f64");

    let mut current_session_status = xcp.get_session_status();

    let mut idle_time = 0.0;
    while RUN.load(Ordering::Acquire) {
        // @@@@ Dev: Terminate mainloop for shutdown if calibration parameter run is false, for test automation
        if !calseg.run {
            break;
        }
        thread::sleep(Duration::from_millis(calseg.cycle_time_ms as u64));

        // Variables on stack and heap
        if xcp.is_connected() {
            mainloop_counter1 += 1;
        }
        *mainloop_counter2 += 1;
        mainloop_map[0][0] = mainloop_counter1 as u8;

        // Capture variable from heap
        daq_capture!(mainloop_counter2, mainloop_event);

        // Measure a 2D map variable directly from heap with an individual event "mainloop_array"
        daq_event_ref!(mainloop_map, RegistryDataType::AUint64, 16, 16, "2D map on heap");

        mainloop_event.trigger();

        // Measure static variables
        static_vars.test_u32 += 1;
        static_vars.test_f64 += 0.1;

        static_event.trigger();

        // Sync
        calseg.sync();

        // Check if the XCP server is still alive
        // Optional
        if !xcp.check_server() {
            warn!("XCP server shutdown!");
            break;
        }

        // Check if the XCP session status has changed and print info
        let session_status = xcp.get_session_status();
        if session_status != current_session_status {
            info!("XCP session status: {:?}", session_status);
            current_session_status = session_status;
        }

        // Log idle time
        if !xcp.is_connected() {
            idle_time += calseg.cycle_time_ms as f64 / 1000.0;
        } else {
            idle_time = 0.0;
        }
        // @@@@ Dev:
        // Finalize A2l after 2s delay
        // This is just for testing, to force creation of A2L file for inspection
        // Without this, the A2L file will be automatically written on XCP connect, to be available for download by CANape
        if idle_time >= 2.0 {
            // Test A2L write
            xcp.write_a2l().unwrap();

            // Test init request
            // xcp.set_init_request();

            // Test freeze request
            // xcp.set_freeze_request();
        }

        // Terminate after more than 10s disconnected to test shutdown behaviour
        // if idle_time >= 10.0 {
        //     break;
        // }
    }

    info!("Main task finished");
    RUN.store(false, Ordering::Relaxed);

    // Wait for the threads to finish
    t1.join().ok().unwrap();
    t.into_iter().for_each(|t| t.join().ok().unwrap());
    info!("All tasks finished");

    // Stop and shutdown the XCP server
    info!("Stop XCP server");
    xcp.stop_server();
}
