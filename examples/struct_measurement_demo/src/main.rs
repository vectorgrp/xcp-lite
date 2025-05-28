// struct_measurement_demo
// Demonstrates measurement of nested structs

/* #region imports */

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::net::Ipv4Addr;
use std::{
    f64::consts::PI,
    time::{Duration, Instant},
};

use xcp_lite::metrics::*;
use xcp_lite::registry::*;
use xcp_lite::*;

/* #endregion */

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "struct_measurement_demo";

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME: u32 = 1000; // 1ms

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

/* #region calibration parameters: semantic data annotation and parameter wrapping */

// To make the parameters adjustable:
// * Add the XcpTypeDescription derive macro to enable automatic registration of the struct and its field attributes
// * Add attributes to describe the parameters
// * Parameter structs must be Copy

#[derive(XcpTypeDescription, Copy, Clone, Serialize, Deserialize, Debug)]
struct Parameters {
    #[characteristic(comment = "Cycle time of the mainloop", min = "100", max = "10000", unit = "us")]
    mainloop_cycle_time: u32,
    #[characteristic(comment = "Counter wraparound", min = "0", max = "10000")]
    counter_max: u16,
    #[characteristic(comment = "Amplitude of the sine signal", min = "0.0", max = "500.0", unit = "Volt")]
    ampl: f64,
    #[characteristic(comment = "Period of the sine signal", min = "0.001", max = "10.0", unit = "s")]
    period: f64,
}

const PARAMETERS_DEFAULTS: Parameters = Parameters {
    ampl: 16.0,
    period: 10.0,
    mainloop_cycle_time: MAINLOOP_CYCLE_TIME,
    counter_max: 2000,
};

// Create a static calibration parameter cell, which can be shared between threads
static PARAMETERS: std::sync::OnceLock<CalCell<Parameters>> = std::sync::OnceLock::new();

/* #endregion */

//-----------------------------------------------------------------------------
// Demo measurement variables

/* #region code instrumentation for measurement: semantic data annotation */
// Code instrumentation for measurement and calibration, to make the data structures observable:
// * Add the XcpTypeDescription derive macro to enable measurement support
// * Add attributes to describe the measurement variables

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Counters {
    #[measurement(comment = "counter", min = "0.0", max = "1000.0")]
    a: i16,
    #[measurement(comment = "counter*2", min = "0.0", max = "2000.0")]
    b: u64,
    #[measurement(comment = "counter*3", min = "0.0", max = "3000.0")]
    c: f64,
}

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Point {
    #[measurement(comment = "x-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    x: f32,
    #[measurement(comment = "y-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    y: f32,
    #[measurement(comment = "z-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    z: f32,
}

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Data {
    // Scalar value with annotations for min, max, conversion rule, physical unit, ...
    #[measurement(comment = "cpu temperature in grad celcius", min = "-50", max = "150", offset = "-50.0", unit = "deg/celcius")]
    cpu_temperature: u8,

    #[measurement(comment = "A 3D vector")]
    vector: Point,

    #[measurement(comment = "Array of 8 points")]
    point_array: [Point; 8],

    #[measurement(comment = "Matrix of 16*16 float values")]
    float_matrix: [[f32; 32]; 32],
}

const DATA_DEFAULT: Data = Data {
    cpu_temperature: 22,
    vector: Point { x: 0.0, y: 0.0, z: 0.0 },
    point_array: [
        Point { x: -10.0, y: 10.0, z: 10.0 },
        Point { x: 10.0, y: 10.0, z: 10.0 },
        Point { x: -10.0, y: -10.0, z: 10.0 },
        Point { x: 10.0, y: -10.0, z: 10.0 },
        Point { x: -10.0, y: 10.0, z: -10.0 },
        Point { x: 10.0, y: 10.0, z: -10.0 },
        Point { x: -10.0, y: -10.0, z: -10.0 },
        Point { x: 10.0, y: -10.0, z: -10.0 },
    ],
    float_matrix: [[0.0; 32]; 32],
};

/* #endregion */

/* #region Rotate a 3D point around (0,0,0) in the cordinate system */
impl Point {
    /// Rotates the point around the x, y, and z axes by the given angles (in radians).
    fn rotate(&self, angle_x: f32, angle_y: f32, angle_z: f32) -> Point {
        // Precompute sine and cosine values for each axis
        let (sin_x, cos_x) = (angle_x.sin(), angle_x.cos());
        let (sin_y, cos_y) = (angle_y.sin(), angle_y.cos());
        let (sin_z, cos_z) = (angle_z.sin(), angle_z.cos());

        // Rotate around the X-axis
        let rotated_x = Point {
            x: self.x,
            y: self.y * cos_x - self.z * sin_x,
            z: self.y * sin_x + self.z * cos_x,
        };

        // Rotate around the Y-axis
        let rotated_y = Point {
            x: rotated_x.x * cos_y + rotated_x.z * sin_y,
            y: rotated_x.y,
            z: -rotated_x.x * sin_y + rotated_x.z * cos_y,
        };

        // Rotate around the Z-axis
        Point {
            x: rotated_y.x * cos_z - rotated_y.y * sin_z,
            y: rotated_y.x * sin_z + rotated_y.y * cos_z,
            z: rotated_y.z,
        }
    }
}

/* #endregion */

//-----------------------------------------------------------------------------
// Main function

fn main() -> Result<()> {
    // Args
    let args = Args::parse();
    let log_level = match args.log_level {
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        5 => log::LevelFilter::Trace,
        _ => log::LevelFilter::Error,
    };

    /* #region INIT_LOGGING */
    // Logging
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();
    /* #endregion */

    // Define some local demo data instances (on stack) to be measured by CANape
    let mut counter1: u64 = 0; // Single value
    let mut counter2: u32 = 1;
    let mut counters: Counters = Counters { a: 0, b: 0, c: 0.0 }; // Single struct
    let mut data: Data = DATA_DEFAULT; // Nested structs and arrays

    /* #region CODE_INSTRUMENTATION */
    //-----------------------------------------------------------------------------

    // Initialize an XCP server
    let app_name = args.name.as_str();
    let app_revision = build_info::format!("{}", $.timestamp);
    let xcp = Xcp::get()
        .set_app_name(app_name)
        .set_app_revision(app_revision)
        .set_log_level(args.log_level)
        .start_server(
            if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
            args.bind.octets(),
            args.port,
            XCP_QUEUE_SIZE,
        )?;

    // Calibration
    // To make a struct of parameters adjustable by CANape, create a calibration segment wrapper for them
    // The parameters can be accessed through deref, which is almost zero cost and lock-free
    let params = PARAMETERS.get_or_init(|| CalCell::new("struct_params", &PARAMETERS_DEFAULTS)).clone_calseg();
    params.register_typedef(); // Register all struct fields as a typedef (with meta data from annotations)

    // Measurement
    // Register measurement values and create an event for the main loop
    // Create event and register measurement variables
    let event = daq_create_event!("main_loop");
    daq_register!(counter1, event, "mainloop counter 1", "");
    daq_register!(counter2, event, "mainloop counter 2", "");
    daq_register_struct!(counters, event);
    daq_register_struct!(data, event);

    //-----------------------------------------------------------------------------
    /* #endregion */

    // Mainloop
    let start_time = Instant::now();
    loop {
        let params = params.read_lock();

        // Modify some demo data
        counter1 += 1;
        counter2 += 1;
        if counter1 > params.counter_max as u64 {
            counter1 = 0;
            counter2 = 1;
        }

        /* #region Modify some more demo data */
        // Different basic data types
        counters.a = counter1 as i16; // 16 bit signed integer
        counters.b = counter1 * 2; // 64 bit unsigned integer
        counters.c = (counter1 * 3) as f64; // 8 byte floating point number

        // Temperature value coded as byte and 0 = -50deg
        data.cpu_temperature = 70 + counter1 as u8 / 100; // 50 = 20deg 

        // 3D point and array[8] of points
        let time_s = start_time.elapsed().as_secs() as f64;
        data.vector.x = (0.001 * (PI * time_s / 3.0).sin()) as f32;
        data.vector.y = (0.001 * (PI * time_s / 3.0).cos()) as f32;
        data.vector.z = 0.0;
        for i in 0..8 {
            data.point_array[i] = data.point_array[i].rotate(data.vector.x, data.vector.y, data.vector.z); // Rotate the point around the x, y, and z axes
        }

        // 32*32 matrix
        for i in 0..32 {
            for j in 0..32 {
                let x = (i as f64) - 16.0;
                let y = (j as f64) - 16.0;
                let phase = (x * x + y * y).sqrt();
                let ampl = params.ampl - phase;
                data.float_matrix[i][j] = (ampl * (PI * time_s / params.period + phase).sin()) as f32;
            }
        }
        /* #endregion */

        /* #region CODE_INSTRUMENTATION */
        //-----------------------------------------------------------------------------

        // Measure the cycle time histogram of the main loop thread
        metrics_histogram!("cycle_time_histogram", 50, 50);
        metrics_counter!("cycle_counter");

        // Consistent, thread safe and lock-free measurement data collection
        // Trigger a timestamped measurement data acquisition event
        event.trigger();

        xcp.finalize_registry().unwrap(); // Write the A2L file once here, for testing purposes only

        //-----------------------------------------------------------------------------
        /* #endregion */

        // Sleep some time and loop endlessly
        std::thread::sleep(Duration::from_micros(params.mainloop_cycle_time as u64)); // us
    }
}
