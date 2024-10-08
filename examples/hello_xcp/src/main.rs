// hello_xcp

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};

use xcp::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Calibration parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage {
    #[type_description(comment = "Max counter value")]
    #[type_description(min = "0")]
    #[type_description(max = "1023")]
    max: u16,

    #[type_description(comment = "Min counter value")]
    #[type_description(min = "0")]
    #[type_description(max = "1023")]
    min: u16,

    #[type_description(comment = "Task delay time in us")]
    #[type_description(min = "0")]
    #[type_description(max = "1000000")]
    #[type_description(unit = "us")]
    delay: u32,
}

// Default value for the calibration parameters
const CAL_PAGE: CalPage = CalPage { min: 5, max: 10, delay: 100000 };

//-----------------------------------------------------------------------------

fn main() {
    println!("XCP Demo");

    env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();

    // Initalize the XCP server
    let xcp = XcpBuilder::new("hello_xcp")
        .set_log_level(XcpLogLevel::Info)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
        .unwrap();

    // Create a calibration segment with default values and register the calibration parameters
    let mut calseg = xcp.create_calseg("calseg", &CAL_PAGE);
    calseg.register_fields();

    // Load calibration parameter mutable page from a file if it exists, otherwise initially save the defaults
    if calseg.load("hello_xcp.json").is_err() {
        calseg.save("hello_xcp.json").unwrap();
    }

    // Measurement signal
    let mut counter: u16 = calseg.min;

    // Register a measurement event and bind it to the measurement signal
    let event = daq_create_event!("mainloop");
    // Register a measurement signal
    daq_register!(counter, event);

    loop {
        counter += 1;
        if counter > calseg.max {
            counter = calseg.min;
        }

        // Trigger timestamped measurement data acquisition
        event.trigger();

        // Synchronize calibration parameters
        calseg.sync();

        xcp.write_a2l().unwrap(); // Force writing the A2L file once (optional, just for inspection)

        thread::sleep(Duration::from_micros(calseg.delay as u64));
    }
}
