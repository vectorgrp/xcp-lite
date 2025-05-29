// main
// xcp-lite test application
//
// Demonstrates the usage of various xcp-lite for Rust features together with a CANape project
// For comprehensive examples better look at the ./examples/ folder

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use std::{
    f64::consts::PI,
    fmt::Debug,
    net::Ipv4Addr,
    num::Wrapping,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

//-----------------------------------------------------------------------------
// xcp_lite lib

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "xcp-lite";

const TASK1_CYCLE_TIME_US: u32 = 10000; // 10ms
const TASK2_CYCLE_TIME_US: u32 = 1000; // 1ms
const TASK2_INSTANCE_COUNT: usize = 10;
const MAINLOOP_CYCLE_TIME: u32 = 100; // 100ms

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB

//-----------------------------------------------------------------------------
// Command line arguments

const DEFAULT_LOG_LEVEL: u8 = 3; // (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
const DEFAULT_BIND_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);
const DEFAULT_PORT: u16 = 5555;
const DEFAULT_TCP: bool = false; // UDP

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = DEFAULT_LOG_LEVEL)]
    log_level: u8,

    /// Bind address, default is ANY
    #[arg(short, long, default_value_t = DEFAULT_BIND_ADDR)]
    bind: Ipv4Addr,

    /// Use TCP as transport layer, default is UDP
    #[arg(short, long, default_value_t = DEFAULT_TCP)]
    tcp: bool,

    /// Port number
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,

    /// Application name
    #[arg(short, long, default_value_t = String::from(APP_NAME))]
    name: String,
}

//-----------------------------------------------------------------------------
// Calibration example
// This approach uses the segment oriented calibration approach with a calibration segment wrapper type
// It provides defined behavior, thread safety and data consistency
// Fields may be automatically added to the A2L registry by the XcpTypeDescription derive macro
// Each page defines a MEMORY_SEGMENT in A2L and in CANape
// A2l addresses are relative to the segment struct start address, the segment number is coded in the address
// Use Serialize, Deserialize (feature=json) for json file persistency

//---------------------------------------------------
// CalPage

#[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
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

#[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
struct TestStruct2 {
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

#[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
struct TestStruct1 {
    test_bool: bool,
    test_u8: u8,
    test_u16: u16,
    test_u32: u32,
    test_u64: u64,
    test_i8: i8,
    test_i8_array: [i8; 4],
    test_i16: i16,
    test_i32: i32,
    test_i64: i64,
    test_f32: f32,
    test_f64: f64,
    test_struct: TestStruct2,
}

#[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
struct CalPage1 {
    counter_max: u32,

    // Basic types
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

    // Nested struct with basic types
    test_struct: TestStruct1,

    // Arrays
    test_struct_array: [u8; 8],
    test_struct_matrix: [[u8; 8]; 16],
}

const CAL_PAGE1: CalPage1 = CalPage1 {
    counter_max: 1000u32,
    test_bool: false,
    test_u8: 8,
    test_u16: 16,
    test_u32: 32,
    test_u64: 64,
    test_i8: -8,
    test_i16: -16,
    test_i32: -32,
    // @@@@ ISSUE CANape does not support negative i64 values
    test_i64: 64, //-1i64,
    test_f32: 0.32,
    test_f64: 0.64,

    test_struct: TestStruct1 {
        test_bool: false,
        test_u8: 18,
        test_u16: 116,
        test_u32: 132,
        test_u64: 164,
        test_i8: -18,
        test_i8_array: [-1i8, 2i8, -3i8, 4i8],
        test_i16: -116,
        test_i32: -132,
        // @@@@ ISSUE CANape does not support negative i64 values
        test_i64: 164,
        test_f32: 1.32,
        test_f64: 1.64,

        test_struct: TestStruct2 {
            test_bool: true,
            test_u8: 28,
            test_u16: 216,
            test_u32: 232,
            test_u64: 264,
            test_i8: -28,
            test_i16: -216,
            test_i32: -232,
            // @@@@ ISSUE CANape does not support negative i64 values
            test_i64: 264,
            test_f32: 2.32,
            test_f64: 2.64,
        },
    },
    test_struct_array: [0, 1, 2, 3, 4, 5, 6, 7],
    test_struct_matrix: [[0, 1, 2, 3, 4, 5, 6, 7]; 16],
};

//---------------------------------------------------
// CalPage2
#[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
struct CalPage2 {
    #[characteristic(comment = "Amplitude")]
    #[characteristic(unit = "Volt")]
    #[characteristic(min = "0")]
    #[characteristic(max = "400")]
    #[characteristic(step = "10")]
    ampl: f64, // VALUE type

    #[characteristic(comment = "Period")]
    #[characteristic(unit = "s")]
    #[characteristic(min = "0")]
    #[characteristic(max = "1000")]
    #[characteristic(step = "20")]
    period: f64, // VALUE type

    #[characteristic(qualifier = "volatile", comment = "Demo array", unit = "ms", min = "0", max = "100")]
    array: [f64; 16], // CURVE type (1 dimension)

    #[axis(comment = "Demo shared axis for curve1/2", min = "0", max = "100")]
    curve_axis: [f32; 16], // AXIS_PTS type

    #[characteristic(comment = "Demo curve", axis = "calseg2.curve_axis", min = "-100", max = "100")]
    curve1: [f64; 16], // CURVE type (1 dimension), shared axis 'shared_axis_16'
    #[characteristic(comment = "Demo curve", axis = "calseg2.curve_axis", min = "-100", max = "100")]
    curve2: [f64; 16], // CURVE type (1 dimension)

    #[axis(comment = "Demo shared axis for map2", min = "0", max = "1000")]
    map_x_axis: [u16; 9], // AXIS_PTS type
    #[characteristic(comment = "Demo map", x_axis = "calseg2.map_x_axis", unit = "ms", min = "0", max = "100")]
    map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)
}

const CAL_PAGE2: CalPage2 = CalPage2 {
    ampl: 100.0,
    period: 1.0,
    array: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    curve_axis: [0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0, 50000.0],
    curve1: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    curve2: [1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.3, 2.4, 2.5],
    map_x_axis: [0, 25, 50, 75, 100, 250, 500, 750, 1000],
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
    log::info!("{} ({:?}) started", thread::current().name().unwrap(), thread::current().id());

    // Create an event instance for each thread, with 8 byte capture buffer
    let mut instance_event = daq_create_event_tli!("main_task2", 8);

    // Create one static event for all instances of this thread, with 8 byte capture buffer
    let mut event = daq_create_event!("main_task2s", 8);

    while RUN.load(Ordering::Relaxed) {
        // Stop task if calibration parameter run2 is false
        if !calseg.read_lock().run2 {
            break;
        }

        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(TASK2_CYCLE_TIME_US as u64));

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

        //metrics_histogram_tli!("task2_cycle_time", 50, 100); // Measure the cycle time of this thread in a histogram 0-5000us, 100us steps
    }
    log::info!("{} stopped", thread::current().name().unwrap());
}

// A task with a single instance which calculates some counter signals of basic types and calibratable sawtooth counter
fn task1(calseg: CalSeg<CalPage>, calseg1: CalSeg<CalPage1>) {
    log::info!("task1 ({:?}) started", thread::current().id());

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

    // Create an event with capture capacity of 1024 bytes for point_cloud serialization
    let event = daq_create_event!("main_task1");

    // Register signals of basic types or array to be captured directly from stack
    daq_register!(counter, event, "", "", 1.0, 0.0);
    daq_register!(counter_i8, event, "counter: i8", "");
    daq_register!(counter_u8, event, "counter: u8", "");
    daq_register!(counter_u16, event, "counter: u16", "");
    daq_register!(counter_i16, event, "counter: i16", "");
    daq_register!(counter_u32, event, "counter: u32", "");
    daq_register!(counter_i32, event, "counter: i32", "");
    daq_register!(counter_u64, event, "counter: u64", "");
    daq_register!(counter_i64, event, "counter: i64", "");
    daq_register!(counter_usize, event, "counter: u64", "");
    daq_register!(counter_isize, event, "counter: i64", "");
    daq_register!(counter_option_u16, event, "counter optional: u8", "");
    daq_register_array!(array1, event);

    while RUN.load(Ordering::Relaxed) {
        // Stop task if calibration parameter run1 is false
        if !calseg.read_lock().run1 {
            break;
        }

        thread::sleep(Duration::from_micros(TASK1_CYCLE_TIME_US as u64));

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
    log::info!("task1 stopped");
}

//-----------------------------------------------------------------------------
// Demo application main

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct TestStruct {
    a: u8,
    b: u64,
    c: f64,
}
#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct TestNestedStruct {
    s1: TestStruct,
    s2: TestStruct,
}

#[allow(unused_assignments)]
fn main() {
    println!("XCP for Rust demo - main- CANape project in ./CANape");

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
    let app_name = args.name.as_str();
    let app_revision = build_info::format!("{}", $.timestamp);
    let xcp = Xcp::get()
        .set_app_name(app_name)
        .set_app_revision(app_revision) // Create new EPK from build info timestamp
        .set_log_level(args.log_level);

    let _xcp = xcp
        .start_server(
            if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
            args.bind.octets(),
            args.port,
            XCP_QUEUE_SIZE,
        )
        .expect("could not start server");

    // Create and register calibration parameter segments (with memory segments in A2L)
    // Calibration segments have "static" lifetime, the Xcp singleton holds a smart pointer clone to each
    // When a calibration segment is dropped by the application and sync is no longer called, the XCP tool will get a timeout when attempting to access it
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (feature freeze), reinitialized from default FLASH page (XCP copy_cal_page)

    // Create a calibration segment wrapper for CAL_PAGE
    // runx, cycle_time_ms
    let calseg = CalSeg::new(
        "calseg",  // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values with static lifetime
    );
    calseg.register_fields();

    if calseg.load("xcp-lite_calseg.json").is_err() {
        calseg.save("xcp-lite_calseg.json").expect("could not write json");
    }

    // Option3: Create a calibration segment wrapper, register with typedef and instance
    let calseg1 = CalSeg::new("calseg1", &CAL_PAGE1);
    calseg1.register_typedef();
    if calseg1.load("xcp-lite_calseg1.json").is_err() {
        calseg1.save("xcp-lite_calseg1.json").expect("could not write json");
    }

    // Create a calibration segment wrapper, register all fields with flattened, mangled instance names, no typedefs
    // Basic types and arrays with attributes
    let calseg2 = CalSeg::new("calseg2", &CAL_PAGE2);
    calseg2.register_fields();
    if calseg2.load("xcp-lite_calseg2.json").is_err() {
        calseg2.save("xcp-lite_calseg2.json").expect("could not write json");
    }

    // Task2 - 9 instances
    // Create multiple tasks which have local or thread local measurement signals
    // To demonstrate the difference between single instance and multi instance events and measurement values
    let mut t2 = Vec::with_capacity(TASK2_INSTANCE_COUNT);
    for instance_num in 0..TASK2_INSTANCE_COUNT {
        let calseg = CalSeg::clone(&calseg);
        let calseg2 = CalSeg::clone(&calseg2);
        let name = format!("task2_{}", instance_num);
        let t = thread::Builder::new()
            .stack_size(32 * 1024)
            .name(name)
            .spawn(move || {
                task2(instance_num, calseg, calseg2);
            })
            .unwrap();
        t2.push(t);
    }

    // Task1 - single instance
    let t1 = thread::spawn({
        let calseg = CalSeg::clone(&calseg);
        move || {
            task1(calseg, calseg1);
        }
    });

    // Create measurement variables on stack
    let mut mainloop_counter1: u64 = 0;
    let mut mainloop_struct1: TestStruct = TestStruct { a: 1, b: 2, c: 3.0 };
    let mut mainloop_struct2: TestStruct = TestStruct { a: 4, b: 5, c: 6.0 };
    let mut mainloop_struct: TestNestedStruct = TestNestedStruct {
        s1: TestStruct { a: 11, b: 12, c: 13.0 },
        s2: TestStruct { a: 21, b: 22, c: 23.0 },
    };

    // Create event and register measurement variables
    let mainloop_event = daq_create_event!("main");
    daq_register!(mainloop_counter1, mainloop_event);
    daq_register_struct!(mainloop_struct1, mainloop_event);
    daq_register_struct!(mainloop_struct2, mainloop_event);
    daq_register_struct!(mainloop_struct, mainloop_event);

    // Mainloop
    xcp_println!("Main task starts");
    while RUN.load(Ordering::Relaxed) {
        if !calseg.read_lock().run {
            break;
        }
        thread::sleep(Duration::from_millis(calseg.read_lock().cycle_time_ms as u64));

        // Variables on stack
        mainloop_counter1 += 1;
        mainloop_struct1.a = mainloop_struct1.a.wrapping_add(1);
        mainloop_struct1.b = mainloop_struct1.b.wrapping_add(1);
        mainloop_struct1.c += 1.0;
        mainloop_struct2.a = mainloop_struct1.a;
        mainloop_struct2.b = mainloop_struct1.b;
        mainloop_struct2.c += 1.0;
        mainloop_struct.s1.a = mainloop_struct1.a;
        mainloop_struct.s1.b = mainloop_struct1.b;
        mainloop_struct.s1.c += 1.0;
        mainloop_struct.s2.a = mainloop_struct1.a;
        mainloop_struct.s2.b = mainloop_struct1.b;
        mainloop_struct.s2.c += 1.0;

        // Variables on stack, current scope
        let mainloop_local_var1 = mainloop_counter1 * 2;
        let mainloop_local_var2 = mainloop_counter1 + 1;
        let mainloop_local_event = daq_create_event!("main_local");
        daq_register!(mainloop_local_var1, mainloop_event, "test local variable with conversion rule", "no unit", 0.5, 10000000.0);
        daq_register!(
            mainloop_local_var2,
            mainloop_local_event,
            "test local variable with conversion rule",
            "no unit",
            2.0,
            -10000000.0
        );

        // Trigger measurement
        mainloop_event.trigger();
        mainloop_local_event.trigger();

        // Check if the XCP server is still alive
        // Optional
        if !xcp.check_server() {
            log::warn!("XCP server shutdown!");
            break;
        }

        let _ = xcp.finalize_registry();
    }

    log::info!("Main task finished");

    // Stop the other tasks
    RUN.store(false, Ordering::Relaxed);

    // Wait for the tasks to finish
    t1.join().unwrap();
    t2.into_iter().for_each(|t| {
        t.join().unwrap();
    });
    log::info!("All tasks finished");

    // Stop and shutdown the XCP server
    log::debug!("Stop XCP server");
    xcp.stop_server();
    log::info!("Server stopped");
}
