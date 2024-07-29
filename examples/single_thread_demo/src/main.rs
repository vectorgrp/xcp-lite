// xcp_lite - single_thread_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    fmt::Debug,
    thread,
    time::{Duration, Instant},
};

use xcp::*;
use xcp_type_description_derive::XcpTypeDescription;

use serde::{Deserialize, Serialize};

//-----------------------------------------------------------------------------
// Demo calibration parameters

// Define a struct with calibration parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage {
    #[comment = "Amplitude of the sine signal"]
    #[unit = "Volt"]
    #[min = "0"]
    #[max = "500"]
    ampl: f64,

    #[comment = "Period of the sine signal"]
    #[unit = "s"]
    #[min = "0.001"]
    #[max = "10"]
    period: f64,

    #[comment = "Counter maximum value"]
    #[min = "0"]
    #[max = "255"]
    counter_max: u32,
}

// Default calibration values
// This will be the FLASH page in the calibration memory segment
const CAL_PAGE: CalPage = CalPage {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Demo application main

fn main() {
    println!("XCPlite Single Thread Demo");

    // Logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Initialize XCP driver singleton, the XCP transport layer server and enable the A2L file creator
    // The A2L file will be finalized on XCP connection and can be uploaded by CANape
    XcpBuilder::new("single_thread_demo")
        .set_log_level(XcpLogLevel::Info) // Set log level of the XCP server
        .enable_a2l(true) // Enabl A2L generation
        .set_epk("EPK_") // Set the EPK string for A2L version check, length must be %4
        .start_server(
            XcpTransportLayer::Udp,
            [127, 0, 0, 1], /*[172, 19, 11, 24]*/
            5555,
            1464,
        )
        .unwrap();

    // Create a calibration parameter set "calseg"
    // This will define a MEMORY_SEGMENT named "calseg" in A2L
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    // The RAM page can be reloaded from a json file (load_json==true)
    // If A2L is enabled (enable_a2l), the A2L description will be generated and provided for upload by CANape
    let calseg = Xcp::create_calseg(
        "calseg",  // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values
        true,      // load RAM page from file "cal_seg".json
    );

    // Mainloop
    let start_time = Instant::now();

    // Measurement variable
    let mut counter: u32 = 0;
    let mut channel_1: f64 = 0.0;

    // Create a measurement event with a unique name "task"
    // This will apear as measurement mode in the CANape measurement configuration
    let event = daq_create_event!("task");

    // Register local variables "counter" and "channel_1" and associate them to event "task"
    daq_register!(counter, event);
    daq_register!(channel_1, event, "sine wave signal", "Volt");

    loop {
        // A saw tooth counter with max from a calibration parameter
        counter += 1;
        if counter > calseg.counter_max {
            counter = 0
        }

        // A sine signal with amplitude and period from calibration parameters
        let time = start_time.elapsed().as_micros() as f64 * 0.000001; // s
        channel_1 = calseg.ampl * (PI * time / calseg.period).sin();
        let _channel_2 = channel_1;

        // Triger the measurement event "task"
        // The measurement event timestamp is taken here and captured data is sent to CANape
        event.trigger();

        // Synchronize calibration operations, if there are any
        // All calibration (mutation of calseg) actions (download, page switch, freeze, init) on segment "calseg" happen here
        calseg.sync();

        thread::sleep(Duration::from_millis(10)); // 100 Hz

        Xcp::get().write_a2l();
    }

    // Stop the XCP server
    // Xcp::stop_server();
}
