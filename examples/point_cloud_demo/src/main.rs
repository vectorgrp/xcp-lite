// xcp_lite - point cloud demo
// Visualize a dynamic list of 3D points in CANape

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use xcp::*;
//use xcp_type_description_derive::XcpTypeDescription;

const BIND_ADDR: [u8; 4] = [192, 168, 0, 83]; // [172, 19, 11, 24]; // [192, 168, 0, 83]; // [127, 0, 0, 1];

const POINT_COUNT: usize = 3;
const AMPL: f64 = 10.0;
const PERIOD: f64 = 3.0;

//-----------------------------------------------------------------------------
// Application start time

lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//---------------------------------------------------------------------------------------

fn main() {
    println!("xcp_lite point cloud demo");

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let xcp = XcpBuilder::new("point_cloud")
        .set_log_level(XcpLogLevel::Debug)
        .enable_a2l(true)
        .start_server(XcpTransportLayer::Udp, BIND_ADDR, 5555, 8000 - 20 - 8)
        .unwrap();

    let mut event_point_cloud = daq_create_event!("point_cloud", 200);

    let mut mainloop_counter1: u64 = 0;
    daq_register!(mainloop_counter1, event_point_cloud);

    loop {
        thread::sleep(Duration::from_millis(50));

        mainloop_counter1 += 1;

        // Serialize a struct into the event capture buffer
        #[derive(Serialize)]
        struct Point {
            x: f32,
            y: f32,
            z: f32,
        }
        let mut point_cloud = Vec::with_capacity(4);
        for i in 0..POINT_COUNT {
            // Calculate demo measurement variable depending on calibration parameters (sine signal with ampl and period)
            let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s

            let x: f32 = (AMPL * (PI * time / PERIOD).sin()) as f32;
            let y: f32 = (AMPL * (PI * time / PERIOD).cos()) as f32;
            let z: f32 = i as f32;
            point_cloud.push(Point { x, y, z });
        }

        daq_serialize!(point_cloud, event_point_cloud, "point cloud demo");
        event_point_cloud.trigger();

        xcp.write_a2l();
    }
}
