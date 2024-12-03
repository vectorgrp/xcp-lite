// xcp-lite - point cloud demo
// Visualize a dynamic list of 3D points in CANape

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    thread,
    time::{Duration, Instant},
};

//-----------------------------------------------------------------------------
// XCP

use xcp::*;

//-----------------------------------------------------------------------------
// Defaults

const BIND_ADDR: [u8; 4] = [127, 0, 0, 1];

const MAX_POINT_COUNT: usize = 20;
const AMPL: f64 = 10.0;
const PERIOD: f64 = 10.0;

const LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
const XCP_LOG_LEVEL: u8 = 3;

//-----------------------------------------------------------------------------
// Parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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

    #[type_description(min = "1")]
    #[type_description(max = "500")]
    point_count: u32,
}

const PARAMS: Params = Params {
    period_x: PERIOD / 2.0,
    ampl_x: AMPL,
    phi_x: 0.0,
    period_y: PERIOD / 4.0,
    ampl_y: AMPL,
    phi_y: 0.0,
    point_count: MAX_POINT_COUNT as u32,
};

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

fn create_point_cloud(params: &CalSeg<Params>) -> PointCloud {
    let mut point_cloud = PointCloud {
        points: Vec::with_capacity(MAX_POINT_COUNT),
    };

    for _ in 0..params.point_count {
        point_cloud.points.push(Point { x: 0.0, y: 0.0, z: 0.0 });
    }
    calculate_point_cloud(&params, &mut point_cloud, 0.0, 0.0, 0.0);
    point_cloud
}

fn calculate_point_cloud(params: &Params, point_cloud: &mut PointCloud, t: f64, phi: f64, h: f64) {
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
    println!("xcp-lite point cloud demo");

    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(LOG_LEVEL).init();

    let xcp = XcpBuilder::new("point_cloud")
        .set_log_level(XCP_LOG_LEVEL)
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555)?;

    let params: CalSeg<Params> = xcp.create_calseg("Params", &PARAMS);
    params.register_fields();

    let mut point_cloud = create_point_cloud(&params);

    let mut event_point_cloud = daq_create_event!("point_cloud", MAX_POINT_COUNT * 12 + 8, 10000000u32);

    info!("Created point cloud: MAX_POINT_COUNT = {}, size = {} bytes", MAX_POINT_COUNT, MAX_POINT_COUNT * 12 + 8);

    let mut counter: u64 = 0;
    daq_register!(counter, event_point_cloud);

    let mut phi = 0.0;
    daq_register!(phi, event_point_cloud);

    let mut h = 0.0;
    daq_register!(h, event_point_cloud);

    let start_time = Instant::now();
    let mut time = 0.0;
    daq_register!(time, event_point_cloud);

    loop {
        thread::sleep(Duration::from_millis(10));
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
        calculate_point_cloud(&params, &mut point_cloud, time, phi, h);

        // Serialize point_cloud into the event capture buffer
        daq_serialize!(point_cloud, event_point_cloud, "point cloud demo");

        // Trigger the measurement event
        event_point_cloud.trigger();

        // Simply recreate the point cloud, when the number of points has changed
        let point_count = params.point_count;
        if params.sync() {
            if params.point_count != point_count {
                point_cloud = create_point_cloud(&params);
            }
        }

        // Write A2L file (once)
        // @@@@ Test, remove
        xcp.write_a2l().unwrap();
    }
    // Ok(())
}
