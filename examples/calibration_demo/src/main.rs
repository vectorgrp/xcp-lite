// calibration_demo
// Calibration of lookup_tables, maps and curves with axis

// Run the demo
// cargo run  --example calibration_demo

#![allow(unused_imports)]

use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::mem::offset_of;
use std::net::Ipv4Addr;
use std::{fmt::Debug, thread, time::Duration};

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "cal_demo";

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
fn cubic_hermite(p0: f32, p1: f32, m0: f32, m1: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * p1 + (t3 - t2) * m1
}

//--------------------------------------------------------------------------------------------------
// Calibration parameters
// Define calibration parameters as structs with semantic annotations provided by XcpTypeDescription

//------------------------------------
// Demo of a simple struct with scalar parameters
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CounterControl {
    #[characteristic(comment = "Start/stop counter")]
    counter_on: bool, // VALUE type

    #[characteristic(comment = "Max counter value")]
    #[characteristic(min = "0", max = "10000")]
    counter_max: u32, // VALUE type
}

// Default values for CounterControl
const COUNTER_CONTROL: CounterControl = CounterControl {
    counter_on: true,
    counter_max: 10000,
};

//--------------------------------------------------------
// Demo of various multi dimensional calibration parameter types in a struct

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct Params {
    #[characteristic(comment = "Demo array", min = "0", max = "100")]
    array: [u8; 4], // VAL_BLK type (1 dimensions)

    #[characteristic(comment = "Demo matrix", min = "0", max = "100")]
    matrix: [[u8; 9]; 5], // VAL_BLK type (2 dimensions)

    #[axis(comment = "Demo shared axis", min = "0", max = "10000")]
    shared_axis_16: [f32; 16], // AXIS_PTS type
    #[axis(comment = "Demo shared axis", min = "0", max = "10000")]
    shared_axis_9: [f32; 9], // AXIS_PTS type

    #[characteristic(comment = "Demo curve with shared axis", axis = "cal_demo_2.params.shared_axis_16", min = "-10", max = "10")]
    curve1: [f64; 16], // CURVE type (1 dimension), shared axis 'shared_axis_16'
    #[characteristic(comment = "Demo curve with shared axis", axis = "cal_demo_2.params.shared_axis_16", min = "-10", max = "10")]
    curve2: [f64; 16], // CURVE type (1 dimension)

    #[characteristic(comment = "Demo map with shared axis", min = "0", max = "100")]
    #[characteristic(x_axis = "cal_demo_2.params.shared_axis_9")]
    #[characteristic(y_axis = "cal_demo_2.params.shared_axis_16")]
    map: [[u8; 9]; 16], // MAP type (2 dimensions), shared axis 'shared_axis_9' and 'shared_axis_16'
}

// Default values for Params
const PARAMS: Params = Params {
    array: [0, 1, 2, 3],
    matrix: [
        [0, 0, 0, 0, 0, 0, 1, 2, 4],
        [0, 0, 0, 0, 0, 0, 2, 3, 5],
        [0, 0, 0, 0, 1, 1, 2, 3, 6],
        [0, 0, 0, 1, 1, 2, 3, 4, 5],
        [0, 0, 1, 2, 3, 5, 7, 8, 10],
    ],
    shared_axis_16: [
        0.0, 20.0, 50.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 800.0, 900.0, 1000.0, 1500.0, 2000.0, 2500.0, 3000.0,
    ],
    shared_axis_9: [0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0],
    curve1: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    curve2: [1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.3, 2.4, 2.5],
    map: [
        [0, 1, 0, 0, 0, 0, 0, 1, 2],
        [2, 3, 0, 0, 0, 0, 0, 2, 3],
        [0, 0, 0, 0, 0, 1, 1, 2, 3],
        [0, 0, 0, 0, 1, 1, 2, 3, 4],
        [0, 0, 1, 1, 2, 3, 4, 5, 7],
        [0, 1, 1, 1, 2, 4, 6, 8, 9],
        [0, 1, 1, 2, 4, 5, 8, 9, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
        [0, 1, 1, 3, 5, 8, 9, 10, 10],
    ],
};

//--------------------------
// Lookup table parameter demo
// For project CANape_typedef this struct is registered as TYPEDEF_STRUCTURE + INSTANCE
// For project CANape_fields this struct is registered as CHARACTERISTIC + AXIS_PTS
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct LookUpTable {
    #[axis(comment = "LookUpTable axis", min = "0", max = "10000")]
    lookup_axis: [f32; 16],
    #[characteristic(comment = "LookUpTable values", axis = "cal_demo_2.lookup_table.lookup_axis", min = "0", max = "10000")]
    lookup_values: [f32; 16],
}

// Implement Default values for LookUpTable
impl Default for LookUpTable {
    fn default() -> Self {
        LookUpTable::DEFAULT
    }
}

// Implement LookUpTable
impl LookUpTable {
    // Default values
    const DEFAULT: LookUpTable = LookUpTable {
        lookup_axis: [
            0.0, 1.0, 2.0, 5.0, 10.0, 220.0, 390.0, 730.0, 1000.0, 1880.0, 2770.0, 4110.0, 5000.0, 7010.0, 8640.0, 10000.0,
        ],
        lookup_values: [0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 530.0, 100.0, 610.0, 210.0, 980.0, 330.0, 730.0, 180.0, 350.0, 0.0],
    };

    // Cubic Hermite spline interpolation output = lookup_spline(input)
    fn lookup_spline(&self, input: f32) -> f32 {
        // Find the interval containing the input
        let mut i = 0;
        while i < self.lookup_axis.len() - 1 && input > self.lookup_axis[i + 1] {
            i += 1;
        }

        // Handle edge cases
        if i == self.lookup_axis.len() - 1 {
            return self.lookup_values[i];
        }

        // Calculate the parameter t in the interval [self.input[i], self.input[i + 1]]
        let t = (input - self.lookup_axis[i]) / (self.lookup_axis[i + 1] - self.lookup_axis[i]);

        // Calculate the slopes (derivatives) at the interval endpoints
        let m0 = if i == 0 {
            (self.lookup_values[i + 1] - self.lookup_values[i]) / (self.lookup_axis[i + 1] - self.lookup_axis[i])
        } else {
            (self.lookup_values[i + 1] - self.lookup_values[i - 1]) / (self.lookup_axis[i + 1] - self.lookup_axis[i - 1])
        };

        let m1 = if i == self.lookup_axis.len() - 2 {
            (self.lookup_values[i + 1] - self.lookup_values[i]) / (self.lookup_axis[i + 1] - self.lookup_axis[i])
        } else {
            (self.lookup_values[i + 2] - self.lookup_values[i]) / (self.lookup_axis[i + 2] - self.lookup_axis[i])
        };

        // Perform cubic Hermite interpolation
        cubic_hermite(self.lookup_values[i], self.lookup_values[i + 1], m0, m1, t)
    }

    // Linear interpolation output = lookup_linear(input)
    fn lookup_linear(&self, input: f32) -> f32 {
        let mut al = self.lookup_axis[0];
        if input <= al {
            al
        } else {
            for (i, an) in self.lookup_axis.iter().enumerate() {
                let an = *an;
                if i > 0 && input <= an {
                    let d = an - al;
                    let f = (input - al) / d;
                    return self.lookup_values[i - 1] + f * (self.lookup_values[i] - self.lookup_values[i - 1]);
                }
                al = an;
            }
            self.lookup_values[self.lookup_values.len() - 1]
        }
    }
}

//-----------------------------------------------
// Calibration data segment2 (A2L MEMORY_SEGMENT)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    // Mainloop delay time
    #[characteristic(comment = "Task delay time in us")]
    #[characteristic(min = "0", max = "2000000", step = "100", unit = "us")]
    delay: u32,

    // Mainloop counter control parameters
    counter_control: CounterControl,
}

// Default values
const CALPAGE1: CalPage1 = CalPage1 {
    delay: MAINLOOP_CYCLE_TIME,
    counter_control: COUNTER_CONTROL,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage2 {
    // Demo of a calibratable lookup table (A2l CURVE with AXIS_PTS)
    // Lookup table output = lookup_table(input)
    lookup_table: LookUpTable,

    // Demo of various other calibration parameter MAP and CURVE types
    params: Params,
}

// Default values
const CALPAGE2: CalPage2 = CalPage2 {
    lookup_table: LookUpTable::DEFAULT,
    params: PARAMS,
};

//-----------------------------------------------------------------------------

#[allow(unused_assignments)]
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

    // XCP: Initialize the XCP server
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

    // XCP: Create calibration segments with default values and register the calibration parameters
    let calseg1 = CalSeg::new("cal_demo_1", &CALPAGE1); // delay, counter_control
    calseg1.register_fields();
    let calseg2 = CalSeg::new("cal_demo_2", &CALPAGE2); // Lookup_table, params
    calseg2.register_fields();

    // XCP: Load calibration parameter page from a file if it exists, otherwise initially save the defaults
    if calseg1.load("cal_demo_seg1.json").is_err() {
        calseg1.save("cal_demo_seg1.json").unwrap();
    }
    if calseg2.load("cal_demo_seg2.json").is_err() {
        calseg1.save("cal_demo_seg2.json").unwrap();
    }

    // Scalar measurement variable counter on stack
    let mut counter: u32 = 0;

    // Struct measurement variable lookup on stack
    #[derive(Clone, Copy, XcpTypeDescription)]
    struct Lookup {
        input: f32,
        output_linear: f32,
        output_spline: f32,
    }
    let mut lookup = Lookup {
        input: 0.0,
        output_linear: 0.0,
        output_spline: 0.0,
    };

    // XCP: Register a measurement event and bind the measurement variables
    let event = daq_create_event!("cal_demo", 16);
    daq_register!(counter, event);
    daq_register_struct!(lookup, event); // Register as nested typedefs and one instance

    let _ = xcp.finalize_registry(); // Force writing of A2L file, otherwise it is written on connect

    loop {
        {
            // XCP: Synchronize calibration parameters in calpage2 and lock read access
            let calpage2 = calseg2.read_lock();

            // Lookup table in calpage2
            // Struct lookup measurement demo
            lookup.input = counter as f32 % 10000.0;
            lookup.output_linear = calpage2.lookup_table.lookup_linear(lookup.input);
            lookup.output_spline = calpage2.lookup_table.lookup_spline(lookup.input);
        }
        {
            // XCP: Synchronize calibration parameters in calpage1 and lock read access
            let calpage1 = calseg1.read_lock();

            // Counter demo
            if calpage1.counter_control.counter_on {
                counter += 1;
                if counter > calpage1.counter_control.counter_max {
                    counter = 0;
                }
            }

            // Sleep for the calibratable mainloop delay time
            thread::sleep(Duration::from_micros(calpage1.delay as u64));
        }

        // XCP: Trigger timestamped measurement data acquisition
        event.trigger();
    }

    // Ok(())
}
