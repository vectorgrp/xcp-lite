// scoped_threads

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread, time::Duration};

use xcp_type_description::prelude::*;
use serde::{Deserialize, Serialize};
use xcp::*;

//-----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage {
    #[comment = "Max counter value"]
    #[min = "0"]
    #[max = "1023"]
    max: u16,

    #[comment = "Min counter value"]
    #[min = "0"]
    #[max = "1023"]
    min: u16,

    #[comment = "Task delay time in us"]
    #[min = "0"]
    #[max = "1000000"]
    #[unit = "us"]
    delay: u32,
}

const CAL_PAGE: CalPage = CalPage {
    min: 5,
    max: 10,
    delay: 100000,
};

fn task1(calseg: &CalSeg<CalPage>) {
    info!("Start task");

    let mut counter: u16 = calseg.min;

    let event = daq_create_event!("main");
    daq_register!(counter, event);

    loop {
        counter += 1;
        if counter > calseg.max {
            counter = calseg.min;
        }
        info!("Counter: {}", counter);
        event.trigger();

        thread::sleep(Duration::from_micros(calseg.delay as u64));

        calseg.sync();
        Xcp::get().write_a2l();
    }
}

//-----------------------------------------------------------------------------

fn main() {
    println!("XCP Demo");

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();

    XcpBuilder::new("xcp_demo")
        .set_log_level(XcpLogLevel::Debug)
        .enable_a2l(true)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1464)
        .unwrap();

    let calseg = Xcp::create_calseg("calseg", &CAL_PAGE, true);

    thread::scope(|s| {
        for _ in 0..2 {
            //     let c = calseg.clone();
            //     s.spawn(|| task1(c));

            // Make sure this does not work
            s.spawn(|| task1(&calseg));
        }
    });
}
