// xcp-lite - point cloud demo
// Visualize a dynamic list of 3D points in CANape

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

//-----------------------------------------------------------------------------
// Defaults

const BIND_ADDR: [u8; 4] = [127, 0, 0, 1];
//const BIND_ADDR: [u8; 4] = [192, 168, 0, 83];
//const BIND_ADDR: [u8; 4] = [172, 19, 11, 24]; ;

const POINT_COUNT: usize = 16;
const AMPL: f64 = 10.0;
const PERIOD: f64 = 10.0;

//-----------------------------------------------------------------------------
// XCP

use xcp::*;
use xcp_idl_generator::prelude::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Application start time

lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Parameters

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct Params {
    #[type_description(unit = "s")]
    #[type_description(min = "0.001")]
    #[type_description(max = "10")]
    period_x: f64,

    #[type_description(unit = "m")]
    #[type_description(min = "0.001")]
    #[type_description(max = "100")]
    ampl_x: f64,

    #[type_description(unit = "PI")]
    #[type_description(min = "0.0")]
    #[type_description(max = "1.0")]
    phi_x: f64,

    #[type_description(unit = "s")]
    #[type_description(min = "0.001")]
    #[type_description(max = "10")]
    period_y: f64,

    #[type_description(unit = "m")]
    #[type_description(min = "0.001")]
    #[type_description(max = "100")]
    ampl_y: f64,

    #[type_description(unit = "PI")]
    #[type_description(min = "0.0")]
    #[type_description(max = "2.0")]
    phi_y: f64,
}

const PARAMS: Params = Params {
    period_x: PERIOD,
    ampl_x: AMPL,
    phi_x: 0.0,
    period_y: PERIOD,
    ampl_y: AMPL,
    phi_y: 0.0,
};

//---------------------------------------------------------------------------------------

#[derive(Debug, Serialize, IdlGenerator)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Serialize, IdlGenerator)]
struct PointCloud {
    points: Vec<Point>,
}

fn create_point_cloud() -> PointCloud {
    let mut point_cloud = PointCloud { points: Vec::with_capacity(4) };

    for _ in 0..POINT_COUNT {
        point_cloud.points.push(Point { x: 0.0, y: 0.0, z: 0.0 });
    }

    point_cloud
}

//---------------------------------------------------------------------------------------

fn main() {
    println!("xcp-lite point cloud demo");

    env_logger::Builder::new().filter_level(log::LevelFilter::Debug).init();

    let xcp = XcpBuilder::new("point_cloud")
        .set_log_level(XcpLogLevel::Debug)
        .enable_a2l(true)
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555)
        .unwrap();

    let params = xcp.create_calseg("Params", &PARAMS, true);

    let mut point_cloud = create_point_cloud();
    let mut event_point_cloud = daq_create_event!("point_cloud", POINT_COUNT * 12 + 8);

    let mut mainloop_counter1: u64 = 0;
    daq_register!(mainloop_counter1, event_point_cloud);

    let mut phi = 0.0;
    let mut h = 0.0;
    loop {
        thread::sleep(Duration::from_millis(10));
        let t = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s

        mainloop_counter1 += 1;
        if mainloop_counter1 > 256 {
            mainloop_counter1 = 0;
        }

        phi += 2.0 * PI / POINT_COUNT as f64 * 0.001;
        if phi > 2.0 * PI / POINT_COUNT as f64 {
            phi = 0.0;
        }
        h += 0.01;
        if h > 20.0 {
            h = 0.0;
        }
        for (i, p) in point_cloud.points.iter_mut().enumerate() {
            let a_x: f64 = params.ampl_x;
            let a_y: f64 = params.ampl_y;
            let omega_x = 2.0 * PI / params.period_x;
            let omega_y = 2.0 * PI / params.period_y;
            let phi_x = 1.8 * PI / POINT_COUNT as f64 * i as f64 + phi;
            let phi_y = 1.8 * PI / POINT_COUNT as f64 * i as f64 + phi;

            p.x = (a_x * (omega_x * t + phi_x).cos()) as f32;
            p.y = (a_y * (omega_y * t + phi_y).sin()) as f32;
            p.z = h + (i as f32 * 0.05);
        }

        // Serialize point_cloud into the event capture buffer
        daq_serialize!(point_cloud, event_point_cloud, "point cloud demo");
        event_point_cloud.trigger();

        params.sync();
        xcp.write_a2l();
    }
}
