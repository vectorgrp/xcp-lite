//-----------------------------------------------------------------------------
// Application xcp
// XCPlite demo for Rust, crate xcp
// (c) 2024 by Vector Informatik GmbH
//
// Demonstrates the usage of xcp-lite for Rust together with a CANape project

// Features:
// json = [] # enable json persistence for CalSeg

// Run:
//  cargo run -- --port 5555 --bind 172.19.11.24 --tcp --no-a2l --log-level 4
//  cargo run -- --port 5555 --bind 192.168.0.83  --segment-size 7972  --log-level 4
//
// Test:
//  Tests may not run in parallel
//  Feature json must be enabled
//  cargo test --features=json -- --test-threads=1 --nocapture

#![allow(dead_code)] // Demo code
#![allow(clippy::vec_init_then_push)]
#![allow(unused_imports)]

use std::{
    f64::consts::PI,
    fmt::Debug,
    sync::{Arc, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

//-----------------------------------------------------------------------------
// Logging

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

//-----------------------------------------------------------------------------
// Command line arguments

use clap::Parser;
use std::net::Ipv4Addr;

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

    /// XCP segment size (jumbo frames supported with MTU up to 8000 bytes -> segment_size <= (OPTION_MTU-20-8), OPTION_MTU defined in main_cfg.h )
    #[arg(short, long, default_value_t = 8000-20-8)]
    segment_size: u16,

    /// Don't create A2L file
    #[arg(short, long, default_value_t = false)]
    no_a2l: bool,
}

//-----------------------------------------------------------------------------
// XCP

use xcp::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Application start time

lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Demo calibration parameter pages

// Definition of structures with calibration parameters
// Implement Serialize, Deserialize for persistence to json
// Implement XcpTypeDescription for auto registration of fields in A2L registry
// Each page defines a MEMORY_SEGMENT in A2L and CANape

//---------------------------------------------------
// CalPage

#[derive(Debug, Clone, Copy, XcpTypeDescription)]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
struct CalPage {
    run: bool,
    run1: bool,
    run2: bool,
    cycle_time_us: u32,
}

const CAL_PAGE: CalPage = CalPage {
    run: true,
    run1: true,
    run2: true,
    cycle_time_us: 1000, // 1ms
};

//---------------------------------------------------
// CalPage1

#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
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
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Max value for counter", min = "0", max = "1000000")]
    counter_max: u32, // This will be a VALUE type

    #[type_description(comment = "Demo curve", unit = "ms", min = "0", max = "100")]
    array: [f64; 16], // This will be a CURVE type (1 dimension)

    #[type_description(comment = "Demo map", unit = "ms", min = "-100", max = "100")]
    map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)

    // Other basic types supported
    test_ints: TestInts,
}

const CAL_PAGE1: CalPage1 = CalPage1 {
    counter_max: 1000,

    array: [
        0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5,
    ],
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

#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage2 {
    #[type_description(comment = "Amplitude")]
    #[type_description(unit = "Volt")]
    #[type_description(min = "0")]
    #[type_description(max = "400")]
    ampl: f64,

    #[type_description(comment = "Period")]
    #[type_description(unit = "s")]
    #[type_description(min = "0")]
    #[type_description(max = "1000")]
    period: f64,
}

const CAL_PAGE2: CalPage2 = CalPage2 {
    ampl: 100.0,
    period: 1.0,
};

//-----------------------------------------------------------------------------
// Demo application cyclic tasks in threads

// A task which calculates some measurement signals depending on calibration parameters in a shared calibration segment
// This task is instantiated multiple times
fn task2(task_id: usize, calseg: CalSeg<CalPage>, calseg2: CalSeg<CalPage2>) {
    // Create events for data acquisition

    // Create an event instance for each thread, with 8 byte capture buffer
    let mut instance_event = daq_create_event_instance!("task2_inst", 8);

    // Create one static event for all instances of this thread, with 8 byte capture buffer
    let mut event = daq_create_event!("task2_static", 8);

    loop {
        // Sleep for a calibratable amount of microseconds, stop task if run is false
        if !calseg.run2 {
            break;
        }
        thread::sleep(Duration::from_micros(calseg.cycle_time_us as u64));

        // Calculate demo measurement variable depending on calibration parameters (sine signal with ampl and period)
        let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
        let offset = task_id as f64 * 10.0;
        let channel = offset + calseg2.ampl * (PI * time / calseg2.period).sin(); // Use active page in calibration segment

        // Measurement of local variables by capturing their value and association to the given XCP event

        // daq_capture_instance adds the event id to the signal name to make the instances of <channel> in different threads unique
        daq_capture_instance!(channel, instance_event, "sine: f64", "Volt");
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

    // Create an event with capture capacity of 1024 bytes for point_cloud serialization
    let event = daq_create_event!("task1");

    // Register signals of bassic types or array to be captured directly from stack
    daq_register!(counter, event, "", "", 1.0, 0.0);
    daq_register!(counter_u8, event, "wrapping counter: u8", "");
    daq_register!(counter_u16, event, "wrapping counter: u16", "");
    daq_register!(counter_u32, event, "wrapping counter: u32", "");
    daq_register!(counter_u64, event, "wrapping counter: u64", "");
    daq_register_array!(array1, event);

    loop {
        if !calseg.run1 {
            break;
        }
        thread::sleep(Duration::from_micros(calseg.cycle_time_us as u64));

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

        // Sync the calibration segment
        calseg1.sync();
        calseg.sync();
    }
    info!("Task1 finished");
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() {
    println!("XCPlite for Rust - CANape Demo (project ./CANape)");

    let args = Args::parse();
    let log_level = XcpLogLevel::from(args.log_level);

    // Logging
    env_logger::Builder::new()
        .filter_level(log_level.to_log_level_filter())
        .init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L writer
    let xcp_builder = XcpBuilder::new("xcp_lite")
        .set_log_level(log_level)
        .enable_a2l(!args.no_a2l)
        // .set_segment_size(1500-20-8) // no jumbo frames
        // .set_epk(build_info::format!("{}", $.timestamp)); // EPK from build info
        .set_epk("EPK_");

    let xcp = match xcp_builder.start_server(
        if args.tcp {
            XcpTransportLayer::Tcp
        } else {
            XcpTransportLayer::Udp
        },
        args.bind.octets(),
        args.port,
        args.segment_size,
    ) {
        Err(res) => {
            error!("XCP server initialization failed: {:?}", res);
            return;
        }
        Ok(xcp) => xcp,
    };

    // Create calibration parameter sets
    // Calibration segments have "static" lifetime, the Xcp singleton holds a smart pointer clone to each
    // When a calibration segment is dropped by the application and sync is no longer called, the XCP tool will get a timeout when attempting to access it
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze), reinitialized from default FLASH page (XCP copy_cal_page)
    // The initial RAM page can be loaded from a json file (load_json=true) or set to the default FLASH page (load_json=false)

    // Create calibration segments for CAL_PAGE, CAL_PAGE1 and CAL_PAGE2
    let mut calseg = Xcp::create_calseg(
        "CalPage", // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values with static lifetime, trait bound from CalPageTrait must be possible
        false,     // load RAM page from file "calseg1".json if existing
    );
    let calseg1 = Xcp::create_calseg("CalPage1", &CAL_PAGE1, true);
    let calseg2 = Xcp::create_calseg("CalPage2", &CAL_PAGE2, true);

    // Task2 - 9 instances
    // To demonstrate the difference between single instance and multi instance events and measurement values
    const INSTANCE_COUNT: usize = 9;
    let mut t = Vec::with_capacity(INSTANCE_COUNT);
    for i in 0..INSTANCE_COUNT {
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

    let mut mainloop_counter1: u64 = 0;
    let mut mainloop_counter2 = Box::new(0u64);
    let mut mainloop_counter3 = Box::new(0u64);
    let mut mainloop_array = Box::new([[0u8; 16]; 16]);

    let mut mainloop_event = daq_create_event!("mainloop", 64); // Capture buffer 64 bytes
    daq_register!(mainloop_counter1, mainloop_event);
    //daq_register_ref!(mainloop_counter2, mainloop_event);

    loop {
        // @@@@ Dev: Terminate mainloop for shutdown if calibration parameter run is false, for test automation
        if !calseg.run {
            break;
        }
        thread::sleep(Duration::from_millis(50));

        mainloop_counter1 += 1;
        *mainloop_counter2 += 2;
        *mainloop_counter3 += 3;
        mainloop_array[0][0] = mainloop_counter1 as u8;

        // Capture variable from heap
        daq_capture!(mainloop_counter2, mainloop_event);
        daq_capture!(mainloop_counter3, mainloop_event);

        // Measure variable directly from heap with individual event "mainloop_array"
        daq_event_ref!(
            mainloop_array,
            RegistryDataType::AUint64,
            16,
            16,
            "array on heap"
        );

        // Measure directly from stack with event "mainloop"
        mainloop_event.trigger();

        // Sync
        calseg.sync();

        // Check if the XCP server is still alive
        // Optional
        if !Xcp::check_server() {
            warn!("XCP server shutdown!");
            break;
        }

        // @@@@ Dev:
        // Finalize A2l after 2s delay
        // This is just for testing, to force immediate creation of A2L file
        // Without this, the A2L file will be automatically written on XCP connect, to be available for download by CANape
        if !args.no_a2l && mainloop_counter1 == 1 {
            thread::sleep(Duration::from_secs(2));
            xcp.write_a2l(); // Test A2L write
                             // xcp.set_init_request(); // Test init request
                             // xcp.set_freeze_request(); // Test freeze request
        }
    }
    info!("Main task finished");

    // @@@@ Dev: Force alls threads to terminate (deref_mut of a calibration segment is undefined behaviour used for testing here)
    calseg.run1 = false;
    calseg.run2 = false;

    // Wait for the threads to finish
    t1.join().ok().unwrap();
    t.into_iter().for_each(|t| t.join().ok().unwrap());
    info!("All tasks finished");

    // Stop and shutdown the XCP server
    Xcp::stop_server();
}
