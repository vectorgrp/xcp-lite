// hello_xcp
// Basis example

// cargo run --example hello_xcp

// Run the test XCP client in another terminal with the following command:
// cargo run --example xcp_client

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};

use xcp::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Calibration parameters
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Max counter value")]
    #[type_description(min = "0")]
    #[type_description(max = "1023")]
    counter_max: u32,

    #[type_description(comment = "Min counter value")]
    #[type_description(min = "0")]
    #[type_description(max = "1023")]
    counter_min: u32,

    #[type_description(comment = "Task delay time in us")]
    #[type_description(min = "0")]
    #[type_description(max = "1000000")]
    #[type_description(unit = "us")]
    delay: u32,
}

// Default value for the calibration parameters
const CAL_PAGE: CalPage1 = CalPage1 {
    counter_min: 5,
    counter_max: 10,
    delay: 100000,
};

//-----------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("XCP Demo");

    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    // Initalize the XCP server
    let xcp = XcpBuilder::new("hello_xcp")
        .set_log_level(3)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)?;

    // Create a calibration segment with default values and register the calibration parameters
    let calseg = xcp.create_calseg("calseg", &CAL_PAGE);
    calseg.register_fields();

    // Load calibration parameter mutable page from a file if it exists, otherwise initially save the defaults
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "serde")]
    if calseg.load("hello_xcp.json").is_err() {
        calseg.save("hello_xcp.json").unwrap();
    }

    // Measurement signal
    let mut counter: u32 = calseg.counter_min;
    let mut counter_u64: u64 = 0;

    // Register a measurement event and bind it to the measurement signal
    let mut event = daq_create_event!("mainloop", 16);

    loop {
        counter += 1;
        counter_u64 += 1;
        if counter > calseg.counter_max {
            counter = calseg.counter_min;
        }

        // Trigger timestamped measurement data acquisition of the counters
        daq_capture!(counter, event);
        daq_capture!(counter_u64, event);
        event.trigger();

        // Synchronize calibration parameters in calseg
        calseg.sync();

        xcp.write_a2l().unwrap(); // Force writing the A2L file once (optional, just for inspection)

        thread::sleep(Duration::from_micros(calseg.delay as u64));
    }

    // Ok(())
}
