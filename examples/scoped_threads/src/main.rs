// scoped_threads

use anyhow::Result;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};

use xcp::*;

//-----------------------------------------------------------------------------

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

const CAL_PAGE: CalPage = CalPage { min: 5, max: 10, delay: 100000 };

fn task1(calseg: CalSeg<CalPage>) {
    info!("Start task");

    let mut counter: u16 = calseg.min;

    let event = daq_create_event!("main");
    daq_register!(counter, event);

    loop {
        counter += 1;
        if counter > calseg.max {
            counter = calseg.min;
        }

        event.trigger();

        thread::sleep(Duration::from_micros(calseg.delay as u64));

        calseg.sync();
    }
}

//-----------------------------------------------------------------------------

fn main() -> Result<()> {
    println!("XCP Demo");

    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    let xcp = XcpBuilder::new("xcp_demo")
        .set_log_level(3)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)?;

    let calseg = xcp.create_calseg("calseg", &CAL_PAGE);
    calseg.register_fields();

    thread::scope(|s| {
        for _ in 0..2 {
            let c = calseg.clone();
            s.spawn(|| task1(c));

            // Make sure this does not work
            // s.spawn(|| task1(&calseg));
        }
    });
    Ok(())
}
