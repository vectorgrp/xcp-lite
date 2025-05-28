// xcp-lite - point cloud demo
// Visualize a dynamic list of 3D points in CANape

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::net::Ipv4Addr;
use std::{
    f64::consts::PI,
    thread,
    time::{Duration, Instant},
};

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "point_cloud";

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME: u32 = 10000; // 10ms

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
// Parameters

const MAX_POINT_COUNT: usize = 20;
const AMPL: f64 = 10.0;
const PERIOD: f64 = 10.0;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct Params {
    #[characteristic(unit = "s")]
    #[characteristic(min = "0.001")]
    #[characteristic(max = "10")]
    period_x: f64,

    #[characteristic(unit = "m")]
    #[characteristic(min = "0.001")]
    #[characteristic(max = "100")]
    ampl_x: f64,

    #[characteristic(unit = "PI")]
    #[characteristic(min = "0.0")]
    #[characteristic(max = "1.0")]
    phi_x: f64,

    #[characteristic(unit = "s")]
    #[characteristic(min = "0.001")]
    #[characteristic(max = "10")]
    period_y: f64,

    #[characteristic(unit = "m")]
    #[characteristic(min = "0.001")]
    #[characteristic(max = "100")]
    ampl_y: f64,

    #[characteristic(unit = "PI")]
    #[characteristic(min = "0.0")]
    #[characteristic(max = "2.0")]
    phi_y: f64,

    #[characteristic(min = "1")]
    #[characteristic(max = "500")]
    point_count: u32,
}

const PARAMS_DEFAULT: Params = Params {
    period_x: PERIOD / 2.0,
    ampl_x: AMPL,
    phi_x: 0.0,
    period_y: PERIOD / 4.0,
    ampl_y: AMPL,
    phi_y: 0.0,
    point_count: MAX_POINT_COUNT as u32,
};

// Create a static cell for the calibration parameters
static PARAMS: std::sync::OnceLock<CalCell<Params>> = std::sync::OnceLock::new();

//---------------------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, IdlGenerator)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, serde::Serialize, IdlGenerator)]
struct PointCloud {
    points: Vec<Point>,
}

fn create_point_cloud() -> PointCloud {
    let params = PARAMS.get().unwrap().clone_calseg();
    let params = params.read_lock();
    let mut point_cloud = PointCloud {
        points: Vec::with_capacity(MAX_POINT_COUNT),
    };

    for _ in 0..params.point_count {
        point_cloud.points.push(Point { x: 0.0, y: 0.0, z: 0.0 });
    }
    calculate_point_cloud(&mut point_cloud, 0.0, 0.0, 0.0);
    point_cloud
}

fn calculate_point_cloud(point_cloud: &mut PointCloud, t: f64, phi: f64, h: f64) {
    let params = PARAMS.get().unwrap().clone_calseg();
    let params = params.read_lock();

    for (i, p) in point_cloud.points.iter_mut().enumerate() {
        let a_x: f64 = params.ampl_x;
        let a_y: f64 = params.ampl_y;
        let omega_x = 2.0 * PI / params.period_x;
        let omega_y = 2.0 * PI / params.period_y;
        let phi_x = 2.0 * PI / MAX_POINT_COUNT as f64 * i as f64 + phi;
        let phi_y = 2.0 * PI / MAX_POINT_COUNT as f64 * i as f64 + phi;

        p.x = (a_x * (omega_x * t + phi_x).cos()) as f32;
        p.y = (a_y * (omega_y * t + phi_y).sin()) as f32;
        //p.z = (h + (i as f64 * 0.05)) as f32;
        p.z = h as f32;
    }
}

//---------------------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("point cloud demo");

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
    let _ = Xcp::get()
        .set_app_name(app_name)
        .set_app_revision(app_revision)
        .set_log_level(args.log_level)
        .start_server(
            if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
            args.bind.octets(),
            args.port,
            XCP_QUEUE_SIZE,
        )?;

    // XCP: Get the calibration parameter set and register all struct fields (with meta data from annotations) in the A2L registry
    let params = PARAMS.get_or_init(|| CalCell::new("point_cloud_params", &PARAMS_DEFAULT)).clone_calseg();
    params.register_fields();

    let mut point_cloud = create_point_cloud();
    let mut counter: u64 = 0;
    let mut phi = 0.0;
    let mut h = 0.0;
    let start_time = Instant::now();
    let mut time = 0.0;
    info!("Created point cloud: MAX_POINT_COUNT = {}, size = {} bytes", MAX_POINT_COUNT, MAX_POINT_COUNT * 12 + 8);

    // XCP: Create a measurement variables and event with capture buffer for the point cloud
    let mut event_point_cloud = daq_create_event!("point_cloud", MAX_POINT_COUNT * 12 + 8);
    daq_register!(counter, event_point_cloud);
    daq_register!(phi, event_point_cloud);
    daq_register!(h, event_point_cloud);
    daq_register!(time, event_point_cloud);

    let mut point_count = params.read_lock().point_count;
    loop {
        thread::sleep(Duration::from_micros(MAINLOOP_CYCLE_TIME as u64));
        time = start_time.elapsed().as_micros() as f64 * 0.000001; // s

        counter += 1;
        if counter > 256 {
            counter = 0;
        }

        phi += 2.0 * PI / MAX_POINT_COUNT as f64 * 0.001;
        if phi > 2.0 * PI / MAX_POINT_COUNT as f64 {
            phi = 0.0;
        }
        h += 0.01;
        if h > 20.0 {
            h = 0.0;
        }
        calculate_point_cloud(&mut point_cloud, time, phi, h);

        // Serialize point_cloud into the event capture buffer
        daq_serialize!(point_cloud, event_point_cloud, "point cloud demo");

        // Trigger the measurement event
        event_point_cloud.trigger();

        // Simply recreate the point cloud, when the number of points has changed
        let new_point_count = params.read_lock().point_count;
        if new_point_count != point_count {
            point_count = new_point_count;
            point_cloud = create_point_cloud();
        }
    }
    // Ok(())
}
