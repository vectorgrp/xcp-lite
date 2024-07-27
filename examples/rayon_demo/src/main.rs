// xcp_lite - rayon demo
// Visualize start and stop of synchronous tasks in worker thread pool
// Compare to xcp_lite tokio_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use std::{thread, time::Duration};

use rayon::prelude::*;

use xcp::*;

// Asynchronous task, sleeps 100ms and ends
fn task(task_index: u16) {
    trace!("task {} start", task_index);

    let event = daq_create_event_instance!("task");

    let mut index = task_index;
    daq_register_instance!(index, event);

    event.trigger();

    thread::sleep(Duration::from_micros(2000));

    index = 0;

    event.trigger();

    trace!("task {} end", index);
}

fn main() {
    println!("xcp_lite_rayon_demo");

    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    XcpBuilder::new("tokio_demo")
        .set_log_level(XcpLogLevel::Info)
        .enable_a2l(true)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1464)
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
