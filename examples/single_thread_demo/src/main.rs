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
// Command line arguments

const DEFAULT_LOG_LEVEL: u8 = 3; // Info
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
// Demo calibration parameters

// Define a struct with calibration parameters
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct Params {
    #[characteristic(comment = "Amplitude of the sine signal in mV")]
    #[characteristic(unit = "mV")]
    #[characteristic(min = "0")]
    #[characteristic(max = "8000")]
    ampl: u16,

    #[characteristic(comment = "Period of the sine signal")]
    #[characteristic(unit = "s")]
    #[characteristic(min = "0.001")]
    #[characteristic(max = "10")]
    period: f64,

    #[characteristic(comment = "Counter maximum value")]
    #[characteristic(min = "0")]
    #[characteristic(max = "255")]
    counter_max: u32,

    #[characteristic(comment = "Task delay time in s, ecu internal value as u32 in us")]
    #[characteristic(min = "0.00001", max = "2", unit = "s", factor = "0.000001")]
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
        let params = params.read_lock();

        // A sine signal with amplitude and period from calibration parameters
        // The value here is the internal value in mV as i16, CANape will convert it to Volt
        let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
        sine = ((params.ampl as f64) * ((PI * time) / params.period).sin()) as i16;
        let _ = sine;

        // Trigger the measurement event
        event.trigger();

        thread::sleep(Duration::from_micros(params.delay as u64));
    }
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() -> Result<()> {
    println!("XCPlite Single Thread Demo");

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

    // XCP: Initialize the XCP server
    let app_name = args.name.as_str();
    let app_revision = build_info::format!("{}", $.timestamp);
    let _xcp = Xcp::get()
        .set_app_name(app_name)
        .set_app_revision(app_revision)
        .set_log_level(args.log_level)
        .start_server(
            if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
            args.bind.octets(),
            args.port,
            XCP_QUEUE_SIZE,
        )?;

    // Create a calibration parameter set "calseg"
    // This will define a MEMORY_SEGMENT named "calseg" in A2L
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable working "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    let params = CalSeg::new(
        "calseg", // name of the calibration segment and the .json file
        &PARAMS,  // default calibration values
    );
    params.register_fields();

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
