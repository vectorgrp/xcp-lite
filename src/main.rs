//-----------------------------------------------------------------------------
// Application xcp
// XCPlite demo for Rust, crate xcp
// (c) 2024 by Vector Informatik GmbH
//
// Demonstrates the usage of xcp-lite for Rust together with a CANape project
// For more comprehensive examples see ./examples/

#![allow(dead_code)] // Demo code
#![allow(clippy::vec_init_then_push)]
#![allow(unused_imports)]

use std::{
    f64::consts::PI,
    fmt::Debug,
    net::Ipv4Addr,
    num::Wrapping,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

//-----------------------------------------------------------------------------

const TASK1_CYCLE_TIME_US: u32 = 10000; // 10ms
const TASK2_CYCLE_TIME_US: u32 = 1000; // 1ms
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

    /// Bind address, default is ANY
    #[arg(short, long, default_value_t = Ipv4Addr::new(0, 0, 0, 0))]
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
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Static measurement variables

// This is the classical address oriented calibration approach for indivual measurement signals

struct StaticVars {
    test_u32: u32,
    test_f64: f64,
}

static STATIC_VARS: static_cell::StaticCell<StaticVars> = static_cell::StaticCell::new();

//-----------------------------------------------------------------------------
// Static calibration variables

// This is the classical address oriented calibration approach for indivual calibration parameters or structs
// The calibration parameters are defined as static instances with constant memory address
// Each variable or struct field has to be registered manually in the A2L registry
// A2L addresses are absolute in the application process memory space (which means relative to the module load address)

// This approach uses a OnceCell to initialize a static instance of calibration data, a mutable static instead would need unnsafe, a static might be in write protected memory and a const has no memory address
// The inner UnsafeCell allows interiour mutability, but this could theoretically cause undefined behaviour or inconsistencies depending on the nature of the platform
// Many C,C++ implementations of XCP do not care about this, but this approach is not recommended for rust projects

struct StaticCalPage {
    task1_cycle_time_us: u32, // Cycle time of task1 in microseconds
    task2_cycle_time_us: u32, // Cycle time of task2 in microseconds
}

static STATIC_CAL_PAGE: once_cell::sync::OnceCell<StaticCalPage> = once_cell::sync::OnceCell::with_value(StaticCalPage {
    task1_cycle_time_us: TASK1_CYCLE_TIME_US,
    task2_cycle_time_us: TASK2_CYCLE_TIME_US,
});

//-----------------------------------------------------------------------------
// Dynamic calibration data example
// This approach uses the segment oriented calibration approach with a calibrastion segment wrapper cell type
// It provides defined behaviour, thread safety and data consistency
// Fields may be automatically added to the A2L registry by the #[derive(serde::Serialize, serde::Deserialize)] feature and the XcpTypeDescription derive macro
// Each page defines a MEMORY_SEGMENT in A2L and CANape
// A2l addresses are relative to the segment start address, the segment number is coded in the address

// Implement Serialize, serde::Deserialize (feature=json) for json file persistency
// Implement XcpTypeDescription (feature=#[derive(serde::Serialize, serde::Deserialize)]) for auto registration of fields in the A2L registry

//---------------------------------------------------
// CalPage

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(XcpTypeDescription, Debug, Clone, Copy)]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(XcpTypeDescription, Debug, Clone, Copy)]
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

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(XcpTypeDescription, Debug, Clone, Copy)]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(XcpTypeDescription, Debug, Clone, Copy)]
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

lazy_static::lazy_static! {

    // Application start time
    static ref START_TIME: Instant = Instant::now();

    // Stop all tasks if false
    static ref RUN: AtomicBool = AtomicBool::new(true);
}

// A task which calculates some measurement signals depending on calibration parameters in a shared calibration segment
// This task is instantiated multiple times
fn task2(instance_num: usize, calseg: CalSeg<CalPage>, calseg2: CalSeg<CalPage2>) {
    info!("{} ({:?}) started", std::thread::current().name().unwrap(), std::thread::current().id());

    // Static calibration parameters
    let static_cal_page = STATIC_CAL_PAGE.get().unwrap();

    // Create an event instance for each thread, with 8 byte capture buffer
    let mut instance_event = daq_create_event_tli!("task2_inst", 8);

    // Create one static event for all instances of this thread, with 8 byte capture buffer
    let mut event = daq_create_event!("task2_static", 8);

    while RUN.load(Ordering::Relaxed) {
        // Stop task if calibration parameter run2 is false
        if !calseg.read_lock().run2 {
            break;
        }

        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(static_cal_page.task2_cycle_time_us as u64));

        let channel = {
            // Synchronize calibration parameters in cal_page and lock read access
            let calseg2 = calseg2.read_lock();

            // Calculate demo measurement variable depending on calibration parameters (sine signal with ampl and period)
            let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
            let offset = instance_num as f64 * 10.0;
            offset + calseg2.ampl * (PI * time / calseg2.period).sin()
        };

        // Measurement of local variables by capturing their value and association to the given XCP event

        // daq_capture_tli adds the event id to the signal name to make the instances of <channel> in different threads unique
        daq_capture_tli!(channel, instance_event, "sine: f64", "Volt");
        instance_event.trigger(); // Take a timestamp and trigger a multi instance data acquisition event

        // daq_capture creates a static signal for all instances of this thread
        daq_capture!(channel, event, "sine: f64", "Volt");
        event.trigger(); // Take a timestamp and trigger the static data acquisition event
    }
    info!("{} stopped", std::thread::current().name().unwrap());
}

// A task with a single instance which calculates some counter signals of basic types and calibratable sawtooth counter
fn task1(calseg: CalSeg<CalPage>, calseg1: CalSeg<CalPage1>) {
    info!("task1 ({:?}) started", std::thread::current().id());

    // Stack variables for measurement
    let mut counter = 0u32;
    let mut counter_u8 = Wrapping(0u8);
    let mut counter_i8 = Wrapping(0i8);
    let mut counter_u16 = Wrapping(0u16);
    let mut counter_i16 = Wrapping(0i16);
    let mut counter_u32 = Wrapping(0u32);
    let mut counter_i32 = Wrapping(0i32);
    let mut counter_u64 = Wrapping(0u64);
    let mut counter_i64 = Wrapping(0i64);
    let mut counter_usize = Wrapping(0usize);
    let mut counter_isize = Wrapping(0isize);
    let mut counter_option_u16: Option<u16> = None;
    let mut array1 = [0.0; 256];
    for (i, a) in array1.iter_mut().enumerate() {
        *a = i as f64;
    }

    // Static calibration parameters
    let static_cal_page = STATIC_CAL_PAGE.get().unwrap();

    // Create an event with capture capacity of 1024 bytes for point_cloud serialization
    let event = daq_create_event!("task1");

    // Register signals of bassic types or array to be captured directly from stack
    daq_register!(counter, event, "", "", 1.0, 0.0);
    daq_register!(counter_i8, event, "wrapping counter: i8", "");
    daq_register!(counter_u8, event, "wrapping counter: u8", "");
    daq_register!(counter_u16, event, "wrapping counter: u16", "");
    daq_register!(counter_i16, event, "wrapping counter: i16", "");
    daq_register!(counter_u32, event, "wrapping counter: u32", "");
    daq_register!(counter_i32, event, "wrapping counter: i32", "");
    daq_register!(counter_u64, event, "wrapping counter: u64", "");
    daq_register!(counter_i64, event, "wrapping counter: i64", "");
    daq_register!(counter_usize, event, "wrapping counter: u64", "");
    daq_register!(counter_isize, event, "wrapping counter: i64", "");
    daq_register!(counter_option_u16, event, "wrapping counter optional: u8", "");
    daq_register_array!(array1, event);

    
    while RUN.load(Ordering::Relaxed) {
        // Stop task if calibration parameter run1 is false
        if !calseg.read_lock().run1 {
            break;
        }

        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(static_cal_page.task1_cycle_time_us as u64));

        let calseg1 = calseg1.read_lock();

        // Basic types and array variables on stack
        counter += 1;
        if counter > calseg1.counter_max {
            counter = 0
        }
        counter_u8 += 1;
        counter_i8 += 1;
        counter_u16 += 1;
        counter_i16 += 1;
        counter_u32 += 1;
        counter_i32 += 1;
        counter_u64 += 1;
        counter_i64 += 1;
        counter_usize += 1;
        counter_isize += 1;
        if counter_option_u16.is_none() {
            counter_option_u16 = Some(counter_u16.0)
        } else {
            counter_option_u16 = None;
        }
        array1[counter_usize.0 % array1.len()] = counter as f64;


        // Trigger single instance event "task1" for data acquisition
        // Capture variables from stack happens here
        event.trigger();
    }
    info!("task1 stopped");
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() {
    println!("XCP for Rust - CANape Demo (project ./CANape)");

    // Args
    let args = Args::parse();
    let log_level = match args.log_level {
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Error,
    };

    // Logging
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();

    // Initialize XCP and start the XCP on ETH server
    let epk = build_info::format!("{}", $.timestamp);
    let xcp = XcpBuilder::new("xcp_lite")
        .set_log_level(args.log_level)
        .set_epk(epk) // Create new EPK from build info timestamp
        .start_server(if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp }, args.bind, args.port)
        .expect("could not start XCP server");

    // Option1: Create and register static calibration variables (from a OnceCell<StaticCalPage>)
    let static_cal_page = STATIC_CAL_PAGE.get().unwrap();
    cal_register_static!(static_cal_page.task1_cycle_time_us, "task1 cycle time", "us");
    cal_register_static!(static_cal_page.task2_cycle_time_us, "task2 cycle time", "us");

    // Create and register calibration parameter segments (with memory segments in A2L)
    // Calibration segments have "static" lifetime, the Xcp singleton holds a smart pointer clone to each
    // When a calibration segment is dropped by the application and sync is no longer called, the XCP tool will get a timeout when attempting to access it
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (feature freeze), reinitialized from default FLASH page (XCP copy_cal_page)

    // Option2: Create a calibration segment wrapper for CAL_PAGE, add fields manually to registry
    let calseg = xcp.add_calseg(
        "CalPage", // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values with static lifetime
    );
    calseg
        .add_field(calseg_field!(CAL_PAGE.run, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.run1, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.run2, 0, 1, "bool"))
        .add_field(calseg_field!(CAL_PAGE.cycle_time_ms, "ms", "main task cycle time"));
    #[cfg(feature = "serde")]
    if calseg.load("xcp-lite_calseg.json").is_err() {
        calseg.save("xcp-lite_calseg.json").expect("could not write json");
    }

    // Option3: Create a calibration segment wrapper add fields automatically with derive macro XcpTypeDescription
    let calseg1 = xcp.create_calseg("CalPage1", &CAL_PAGE1);
    calseg1.register_fields();
    #[cfg(feature = "serde")]
    if calseg1.load("xcp-lite_calseg1.json").is_err() {
        calseg1.save("xcp-lite_calseg1.json").expect("could not write json");
    }
    let calseg2 = xcp.create_calseg("CalPage2", &CAL_PAGE2);
    calseg2.register_fields();
    #[cfg(feature = "serde")]
    if calseg2.load("xcp-lite_calseg2.json").is_err() {
        calseg2.save("xcp-lite_calseg2.json").expect("could not write json");
    }

    // Create multiple tasks which have local or thread local measurement signals

    // Task2 - 9 instances
    // To demonstrate the difference between single instance and multi instance events and measurement values
    let mut t2 = Vec::with_capacity(TASK2_INSTANCE_COUNT);
    for instance_num in 0..TASK2_INSTANCE_COUNT {
        let calseg = CalSeg::clone(&calseg);
        let calseg2 = CalSeg::clone(&calseg2);
        let name = format!("task2_{}", instance_num);
        let t = std::thread::Builder::new()
            .stack_size(32 * 1024)
            .name(name)
            .spawn(move || {
                task2(instance_num, calseg, calseg2);
            })
            .unwrap();
        t2.push(t);
    }

    // Task1 - single instance
    // calseg1 moved, calseg cloned
    let t1 = thread::spawn({
        let calseg = CalSeg::clone(&calseg);
        move || {
            task1(calseg, calseg1);
        }
    });

    // Create measurment variables on heap and stack
    let mut mainloop_counter1: u64 = 0;
    let mut mainloop_map = Box::new([[0u8; 16]; 16]);

    // Create associated event and register
    let mainloop_event = daq_create_event!("mainloop");
    daq_register!(mainloop_counter1, mainloop_event);

    // Mutable static variables (borrowed from a StaticCell<StaticVars>)
    let static_vars: &'static mut StaticVars = STATIC_VARS.init(StaticVars { test_u32: 0, test_f64: 0.0 });
    // Create associated event and register as characteristics with absolute addressing and associated XCP event
    let static_event = xcp.create_event("static_event");
    daq_register_static!(static_vars.test_u32, static_event, "Test static u32");
    daq_register_static!(static_vars.test_f64, static_event, "Test static f64");

    // Mainloop
    xcp_println!("Main task starts");
    while RUN.load(Ordering::Relaxed) {
        if !calseg.read_lock().run {
            break;
        }
        thread::sleep(Duration::from_millis(calseg.cycle_time_ms as u64));

        // Variables on stack and heap
        mainloop_counter1 += 1;
        mainloop_map[0][0] = mainloop_counter1 as u8;

        // Measure a 2D map variable directly from heap with an individual event "mainloop_array"
        daq_event_ref!(mainloop_map, RegistryDataType::AUint64, 16, 16, "2D map on heap");

        mainloop_event.trigger();

        // Local variables on stack
        let mainloop_local_var1 = mainloop_counter1 * 2;
        let mainloop_local_var2 = mainloop_counter1 * 3;
        let mainloop_local_event = daq_create_event!("mainloop_local");
        daq_register!(mainloop_local_var1, mainloop_local_event);
        daq_register!(mainloop_local_var2, mainloop_local_event);
        mainloop_local_event.trigger();

        // Measure static variables
        static_vars.test_u32 += 1;
        static_vars.test_f64 += 0.1;
        static_event.trigger();

        // Check if the XCP server is still alive
        // Optional
        if !xcp.check_server() {
            warn!("XCP server shutdown!");
            break;
        }

        // Create A2L file early
        let _ = xcp.write_a2l();
    }

    info!("Main task finished");
    // Stop the other tasks
    RUN.store(false, Ordering::Relaxed);

    // Wait for the tasks to finish
    t1.join().unwrap();
    t2.into_iter().for_each(|t| {
        t.join().unwrap();
    });
    info!("All tasks finished");

    // Stop and shutdown the XCP server
    info!("Stop XCP server");
    xcp.stop_server();
    info!("Server stopped");
}
