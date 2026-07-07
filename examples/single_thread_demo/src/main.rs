// xcp-lite - single_thread_demo
#![allow(unused_imports)]

use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::net::Ipv4Addr;
use std::{
    f64::consts::PI,
    fmt::Debug,
    thread,
    time::{Duration, Instant},
};

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "single_thread_demo";
const JSON_FILE: &str = "single_thread_demo.json"; // JSON file for calibration segment

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME: u32 = 10000; // 10ms

// Static application start time
lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Command line arguments (shared parser, see examples/common)

use example_common::ExampleArgs;

//-----------------------------------------------------------------------------
// Demo calibration parameters

// Define a struct with calibration parameters
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
struct Params {
    #[characteristic(comment = "Amplitude of the sine signal in mV", unit = "mV", min = 0, max = 8000)]
    ampl: u16,

    #[characteristic(comment = "Period of the sine signal", unit = "s", min = 0.001, max = 10)]
    period: f64,

    #[characteristic(comment = "Counter maximum value", min = 0, max = 255)]
    counter_max: u32,

    #[characteristic(comment = "Task delay time in s, ecu internal value as u32 in us", min = 0.00001, max = 2, unit = "s", factor = 0.000001)]
    delay: u32,
}

// Default calibration values
// This will be the FLASH page in the calibration memory segment
const PARAMS: Params = Params {
    ampl: 1000,  // mV
    period: 5.0, // s
    counter_max: 100,
    delay: MAINLOOP_CYCLE_TIME,
};

//-----------------------------------------------------------------------------
// Demo task

// A task executed in multiple threads sharing a calibration parameter segment
fn task(params: CalSeg<Params>) {
    // Demo signal
    let mut sine: i16 = 0; // mV

    // Create an event and register variables for measurement directly from stack
    let event = daq_create_event!("thread_loop", 16);
    daq_register!(sine, event, "sine wave signal, internal value in mV", "Volt", 0.001, 0.0);

    loop {
        // Lock the calibration segment for read access
        {
            let params = params.read_lock();

            // A sine signal with amplitude and period from calibration parameters
            // The value here is the internal value in mV as i16, CANape will convert it to Volt
            let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
            sine = ((params.ampl as f64) * ((PI * time) / params.period).sin()) as i16;
            let _ = sine;
        }

        // Trigger the measurement event
        event.trigger();

        let delay = params.read_lock().delay; // release the lock before sleeping
        thread::sleep(Duration::from_micros(delay as u64));
    }
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() -> Result<()> {
    println!("XCPlite Single Thread Demo");

    // Args
    let args = ExampleArgs::parse();
    args.init_logging();

    // XCP: Initialize the XCP server
    let app_name = args.app_name(APP_NAME);
    let app_revision = build_info::format!("{}", $.timestamp);
    let _xcp = Xcp::init(app_name, app_revision, args.log_level).start_server(
        if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
        args.bind.octets(),
        args.port,
        XCP_QUEUE_SIZE,
    )?;

    // XCP: Select flattened or typedef A2L representation (--flatten)
    Xcp::get().set_registry_mode(args.flatten, false);

    // Create a calibration parameter set "calseg"
    // This will define a MEMORY_SEGMENT named "calseg" in A2L
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable working "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    let params = CalSeg::new(
        "calseg", // name of the calibration segment and the .json file
        &PARAMS,  // default calibration values
    );
    params.register();

    // Load calseg from json file
    if params.load(JSON_FILE).is_err() {
        params.save(JSON_FILE).unwrap();
    }

    // Create a thread
    thread::spawn({
        // Move a clone of the calibration parameters into the thread
        let params = CalSeg::clone(&params);
        move || {
            task(params);
        }
    });

    // Measurement variable
    let mut counter: u32 = 0;

    // Create a measurement event
    // This will apear as measurement mode in the CANape measurement configuration
    let event = daq_create_event!("main_loop");

    // Register local variables and associate them to the event
    daq_register!(counter, event);

    loop {
        // Lock the calibration segment for read access
        let calseg = params.read_lock();

        // A saw tooth counter with a max value from a calibration parameter
        counter += 1;
        if counter > calseg.counter_max {
            counter = 0
        }

        // Trigger the measurement event
        // The measurement event timestamp is taken here and captured data is sent to CANape
        event.trigger();

        thread::sleep(Duration::from_micros(calseg.delay as u64));

        // Generate the A2L file once immediately
        // xcp.finalize_registry().unwrap();
    }

    // Stop the XCP server
    // Xcp::stop_server();

    // Ok(())
}
