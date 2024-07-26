// xcp_lite - tokio_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use std::{thread, time::Duration};

use rayon::prelude::*;

use xcp::*;
const OPTION_SERVER_ADDR: [u8; 4] = [192, 168, 0, 83]; // Home
                                                       //const OPTION_SERVER_ADDR: [u8; 4] = [172, 19, 11, 24]; // Office 172.19.11.24
                                                       //const OPTION_SERVER_ADDR: [u8; 4] = [127, 0, 0, 1]; // Localhost

// Asynchronous task, sleeps 100ms and ends
fn task(task_index: u16) {
    let mut index = task_index;
    trace!("task {} start", index);

    let mut event = daq_create_event_instance!("task", 256);

    daq_capture_instance!(index, event);
    event.trigger();

    thread::sleep(Duration::from_micros(2000));

    index = 0;
    daq_capture_instance!(index, event);
    event.trigger();

    trace!("task {} end", index);
}

fn main() {
    println!("xcp_lite_rayon_demo");

    // Logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L generation
    XcpBuilder::new("tokio_demo")
        .set_log_level(XcpLogLevel::Info)
        .enable_a2l(true)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, OPTION_SERVER_ADDR, 5555, 1464)
        .unwrap();

    loop {
        (1..=100).into_par_iter().for_each(|i| {
            task(i);
        });

        thread::sleep(std::time::Duration::from_millis(100));
    }

    //Xcp::get().write_a2l();
    //Xcp::stop_server();
}
