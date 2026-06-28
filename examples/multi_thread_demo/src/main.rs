// xcp-lite - multi_thread_demo

#![allow(unused_imports)]

use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::net::Ipv4Addr;
use std::thread::sleep;
use std::{
    f64::consts::PI,
    fmt::Debug,
    thread,
    time::{Duration, Instant},
};

use xcp_lite::registry::*;
use xcp_lite::*;

// Static application start time
lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "multi_thread_demo";

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME_US: u32 = 10000; // 10ms

//-----------------------------------------------------------------------------
// Command line arguments (shared parser, see examples/common)

use example_common::ExampleArgs;

//-----------------------------------------------------------------------------
// Demo calibration parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
struct Params {
    #[characteristic(comment = "Task delay time in s, ecu internal value as u32 in us", min = 0.00001, max = 2, unit = "s", factor = 0.000001)]
    delay: u32,

    #[characteristic(comment = "Amplitude of the sine signal", unit = "Volt", min = 0, max = 500)]
    ampl: f64,

    #[characteristic(comment = "Period of the sine signal", unit = "s", min = 0.001, max = 10)]
    period: f64,

    #[characteristic(comment = "Counter maximum value", min = 0, max = 255, step = 10)]
    counter_max: u32,
}

const CALPAGE1: Params = Params {
    delay: MAINLOOP_CYCLE_TIME_US,
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

// Create a static cell for the calibration segment, which is shared between the threads
// The alternative would be to move a clone of a CalSeg into each thread
static CALSEG1: std::sync::OnceLock<CalCell<Params>> = std::sync::OnceLock::new();

//-----------------------------------------------------------------------------
// Demo task

// A task executed in multiple threads sharing a calibration parameter segment
fn task(id: u32) {
    // Get the static calibration segment
    let calseg1 = CALSEG1.get().unwrap().clone_calseg();

    // Create a thread local event instance
    // The capacity of the event capture buffer is 16 bytes
    let mut event = daq_create_event_tli!("task", 16);
    println!("Task {id} started");

    // Demo signals
    let mut counter: u32 = 0;
    let mut sine: f64;

    let mut sleep_time: u64;
    loop {
        {
            let calseg1 = calseg1.read_lock();

            // A counter wrapping at a value specified by a calibration parameter
            counter += 1;
            if counter > calseg1.counter_max {
                counter = 0
            }

            // A sine signal with amplitude and period from calibration parameters and an offset from thread id
            let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
            sine = (id as f64) * 10.0 + calseg1.ampl * ((PI * time) / calseg1.period).sin();

            sleep_time = calseg1.delay as u64;
        }

        // Register them once for each task instance and associate to the task instance event
        // Copy the value to the event capture buffer
        daq_capture_tli!(counter, event);
        daq_capture_tli!(sine, event, "sine wave signal", "Volt", 1.0, 0.0);

        // Trigger the measurement event
        // Take a event timestamp send the captured data
        event.trigger();

        thread::sleep(Duration::from_micros(sleep_time));
    }
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() -> Result<()> {
    println!("XCPlite Multi Thread Demo");

    // Args
    let args = ExampleArgs::parse();
    args.init_logging();

    // XCP: Initialize the XCP server
    let app_name = args.app_name(APP_NAME);
    let app_revision = build_info::format!("{}", $.timestamp);
    let xcp = Xcp::init(app_name, app_revision, args.log_level).start_server(
        if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
        args.bind.octets(),
        args.port,
        XCP_QUEUE_SIZE,
    )?;

    // XCP: Select flattened or typedef A2L representation (--flatten)
    Xcp::get().set_registry_mode(args.flatten, false);

    // Create a static calibration parameter set (will be a MEMORY_SEGMENT in A2L) from a const struct CALPAGE1
    // The calibration parameters are shared between the threads
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched at runtime (XCP set_cal_page), saved to json (XCP freeze) freeze and reinitialized from FLASH (XCP copy_cal_page)
    let params = CALSEG1.get_or_init(|| CalCell::new("multi_thread_params", &CALPAGE1)).clone_calseg();
    params.register(); // Register all struct fields (with meta data from annotations) in the A2L registry

    // Start multiple instances of the demo task
    // Each instance will create its own measurement variable and event instances
    let mut t = Vec::new();
    for i in 0..=10 {
        t.push(thread::spawn({
            move || {
                task(i);
            }
        }));
    }

    // Test: Generate A2L immediately (normally this happens on XCP tool connect)
    // Wait some time until all threads have registered their measurement signals and events
    thread::sleep(Duration::from_millis(1000));
    xcp.finalize_registry().unwrap();

    // Wait for the threads to finish
    t.into_iter().for_each(|t| t.join().unwrap());

    // Stop the XCP server
    xcp.stop_server();

    Ok(())
}
