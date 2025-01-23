// hello_xcp
// Basic demo

// Run the demo
// cargo run --features serde --example hello_xcp

// Run the test XCP client in another terminal or start CANape with the project in folder examples/hello_xcp/CANape
// cargo run --example xcp_client

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};
use xcp::*;

//-----------------------------------------------------------------------------
// Calibration parameters

// Define calibration parameters as a struct
// XCP: Add meta data for A2L generation
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage {
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

// Optionally define methods if needed
impl CalPage {
    fn get_delay(&self) -> u64 {
        self.delay as u64
    }
}

// Default values for the calibration parameters
const CAL_PAGE: CalPage = CalPage {
    counter_min: 5,
    counter_max: 10,
    delay: 100000,
};

//-----------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("XCP Demo");

    // Logging
    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    // XCP: Initialize the XCP server
    let xcp = XcpBuilder::new("hello_xcp")
        .set_log_level(3)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1024 * 64)?;

    // XCP: Create a calibration segment wrapper with default values and register the calibration parameters
    let cal_page = xcp.create_calseg("calseg", &CAL_PAGE);
    cal_page.register_fields();

    // XCP: Load calibration parameter page from a file if it exists, otherwise initially save the defaults
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "serde")]
    if cal_page.load("hello_xcp.json").is_err() {
        cal_page.save("hello_xcp.json").unwrap();
    }

    // Measurement variables on stack
    let mut counter: u32 = cal_page.counter_min;
    let mut counter_u64: u64 = 0;

    // XCP: Register a measurement event and bind the measurement variables
    let event = daq_create_event!("mainloop", 16);
    daq_register!(counter, event);
    daq_register!(counter_u64, event);

    loop {
        // XCP: Synchronize calibration parameters in cal_page and lock read access
        let cal_page = cal_page.read_lock();

        counter += 1;
        counter_u64 += 1;
        if counter > cal_page.counter_max {
            counter = cal_page.counter_min;
        }

        // XCP: Trigger timestamped measurement data acquisition
        event.trigger();

        thread::sleep(Duration::from_micros(cal_page.get_delay()));
    }

    // Ok(())
}
