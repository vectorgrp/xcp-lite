// hello_xcp
// xcp-lite basic demo
//
// Demonstrates the usage of xcp-lite for Rust together with a CANape project
//
// Run the demo
// cargo run --example hello_xcp
//
// Run the test XCP client in another terminal or start CANape with the project in folder examples/hello_xcp/CANape
// cargo run --example xcp_client -- -m "counter"

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "hello_xcp";
const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME: u32 = 10000; // 10ms

//-----------------------------------------------------------------------------
// Command line arguments

const DEFAULT_LOG_LEVEL: u8 = 3; // Info
const DEFAULT_BIND_ADDR: std::net::Ipv4Addr = std::net::Ipv4Addr::new(0, 0, 0, 0); // ANY
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
    bind: std::net::Ipv4Addr,

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

// Define calibration parameters in a struct with semantic annotations to create the A2L file
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct Params {
    #[characteristic(comment = "Start/stop counter")]
    counter_on: bool,

    #[characteristic(comment = "Max counter value")]
    #[characteristic(min = "0", max = "1023")]
    counter_max: u32,

    #[characteristic(comment = "Task delay time in s, ecu internal value as u32 in us")]
    #[characteristic(min = "0.00001", max = "2", unit = "s", factor = "0.000001")]
    delay: u32,

    #[characteristic(comment = "Demo array", min = "0", max = "100")]
    array: [u8; 4],

    #[characteristic(comment = "Demo matrix", min = "0", max = "100")]
    matrix: [[u8; 8]; 4],
}

// Default values for the calibration parameters
const PARAMS: Params = Params {
    counter_on: true,
    counter_max: 100,
    delay: MAINLOOP_CYCLE_TIME,
    array: [0, 2, 5, 10],
    matrix: [[0, 0, 0, 0, 0, 0, 1, 2], [0, 0, 0, 0, 0, 0, 2, 3], [0, 0, 0, 0, 1, 1, 2, 3], [0, 0, 0, 1, 1, 2, 3, 4]],
};

//-----------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    println!("XCP for Rust demo - hello_xcp - CANape project in ./examples/hello_xcp/CANape");

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
    let app_revision = build_info::format!("EPK_{}", $.timestamp);
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

    // XCP: Create a calibration segment wrapper with default values and register the calibration parameters
    let params = CalSeg::new("my_params", &PARAMS);
    params.register_fields();

    // Demo measurement variable on stack
    let mut counter: u32 = 0;

    // XCP: Register a measurement event and bind measurement variables
    let event = daq_create_event!("my_event", 16);
    daq_register!(counter, event);

    loop {
        // XCP: Synchronize calibration parameters in cal_page and lock read access for consistency
        let params = params.read_lock();

        if params.counter_on {
            counter += 1;
            if counter > params.counter_max {
                counter = 0;
            }
        }

        // XCP: Trigger timestamped measurement data acquisition
        event.trigger();

        std::thread::sleep(std::time::Duration::from_micros(params.delay as u64));
    }
}
