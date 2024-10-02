// hello_xcp

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};

use serde::{Deserialize, Serialize};
use xcp::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Calibration parameters

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
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

const CAL_PAGE: CalPage = CalPage { min: 5, max: 10, delay: 100000 };

//-----------------------------------------------------------------------------

fn main() {
    println!("XCP Demo");

    env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();

    let xcp = XcpBuilder::new("hello_xcp")
        .set_log_level(XcpLogLevel::Info)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
        .unwrap();

    let calseg = xcp.create_calseg("CalPage", &CAL_PAGE, true);

    // Measurement signal
    let mut counter: u16 = calseg.min;

    // Register a measurement event and bind it to the counter signal
    let event = daq_create_event!("mainloop");
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

        thread::sleep(Duration::from_micros(calseg.delay as u64));

        xcp.write_a2l().unwrap(); // Force writing the A2L file once (for inspection)
    }
}
