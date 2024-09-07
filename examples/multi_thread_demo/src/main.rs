// xcp_lite - multi_thread_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    fmt::Debug,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use xcp::*;
use xcp_type_description::prelude::*;

// Static application start time
lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Demo calibration parameters

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Amplitude of the sine signal")]
    #[type_description(unit = "Volt")]
    #[type_description(min = "0")]
    #[type_description(max = "500")]
    ampl: f64,

    #[type_description(comment = "Period of the sine signal")]
    #[type_description(unit = "s")]
    #[type_description(min = "0.001")]
    #[type_description(max = "10")]
    period: f64,

    #[type_description(comment = "Counter maximum value")]
    #[type_description(min = "0")]
    #[type_description(max = "255")]
    counter_max: u32,
}

const CAL_PAGE: CalPage1 = CalPage1 {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Demo task

// A task executed in multiple threads with a calibration segment
fn task(id: u32, cal_seg: CalSeg<CalPage1>) {
    // Create an event instance
    // The event instance is used to trigger the measurement event and capture data
    // The capacity in bytes of the capture buffer may be explictly specified (default would be 256)
    let mut event = daq_create_event_instance!("task1", 16);
    println!("Task started, id = {}", id);

    let mut counter: u32 = 0;
    let mut sine: f64;
    loop {
        thread::sleep(Duration::from_millis(10)); // 100 Hz

        // A simple saw tooth counter with max from a calibration parameter
        counter += 1;
        if counter > cal_seg.counter_max {
            counter = 0
        }

        // A sine signal with amplitude and period from calibration parameters
        let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
        sine = (id as f64) * 10.0 + cal_seg.ampl * ((PI * time / 10.0) / cal_seg.period).sin();

        // Capture local variables and associate to event
        daq_capture_instance!(counter, event);
        daq_capture_instance!(sine, event, "sine: f64", "Volt", 1.0, 0.0);

        // Triger the measurement event
        // Measurement event timestamp is taken here and captured data is sent
        event.trigger();

        // Synchronize calibration operations through the calibration event
        // All calibration actions (download, page switch, freeze, init) on segment "calseg" happen here
        cal_seg.sync();
    }
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() {
    println!("XCPlite Multi Thread Demo");

    // Logging
    env_logger::Builder::new().filter_level(log::LevelFilter::Warn).init();

    // Initialize XCP driver singleton, the transport layer server and enable the registry
    let xcp = XcpBuilder::new("multi_thread_demo")
        .set_log_level(XcpLogLevel::Warn)
        .enable_a2l(true)
        .set_epk("EPK_12345678")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
        .unwrap();

    // Create calibration parameter sets (CalSeg in rust, MEMORY_SEGMENT in A2L) from annotated structs
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be selected runtime (XCP set_cal_page), saves to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    // RAM page can be loaded from json in new
    let calseg = xcp.create_calseg(
        "calseg",  // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values
        true,      // load RAM page from file "cal_seg1".json
    );

    // Start multiple instances of the demo task
    // Each instance will create its own measurement variables and events
    // The calibration segment is shared between the tasks
    let mut t = Vec::new();
    for i in 0..=9 {
        let c = CalSeg::clone(&calseg);
        t.push(thread::spawn(move || {
            task(i, c);
        }));
    }

    thread::sleep(Duration::from_millis(1000));
    Xcp::get().write_a2l();

    t.into_iter().for_each(|t| t.join().unwrap());

    // Stop the XCP server
    xcp.stop_server();
}
