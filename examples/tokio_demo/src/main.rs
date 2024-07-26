// xcp_lite - tokio_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use tokio::{join, time::sleep};

use xcp::*;

const OPTION_SERVER_ADDR: [u8; 4] = [127, 0, 0, 1];

// Asynchronous task, sleeps 100ms and ends
// Demonstrates multi instance measurement
#[allow(dead_code)]
async fn task1(task_index: u16) {
    let mut index: i16 = task_index as i16;
    trace!("task {} start", index);
    let mut event = daq_create_event_instance!("task");
    daq_capture_instance!(index, event, "Task index", "");
    event.trigger();
    sleep(tokio::time::Duration::from_millis(2)).await;
    index = -index;
    daq_capture_instance!(index, event, "Task index", "");
    event.trigger();
    trace!("task {} end", index);
}

// Asynchronous task, sleeps 100ms and ends
// Demonstrates static measurement
#[allow(dead_code)]
async fn task2(task_index: u16) {
    let mut index: i16 = task_index as i16;
    trace!("task {} start", index);
    let mut event = daq_create_event!("task", 8);
    daq_capture!(index, event, "Task index", "");
    event.trigger();
    sleep(tokio::time::Duration::from_millis(2)).await;
    index = -index;
    daq_capture!(index, event, "Task index", "");
    event.trigger();
    trace!("task {} end", index);
}

#[tokio::main]
async fn main() {
    println!("xcp_lite_tokio_demo");

    // Logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L generation
    XcpBuilder::new("tokio_demo")
        .set_log_level(XcpLogLevel::Warn)
        .enable_a2l(true)
        .set_epk("EPK")
        .start_server(XcpTransportLayer::Udp, OPTION_SERVER_ADDR, 5555, 1464)
        .unwrap();

    trace!("Start");

    loop {
        sleep(tokio::time::Duration::from_secs(1)).await;

        let mut tasks = Vec::new();

        const N: u16 = 100;
        for i in 1..=N {
            tasks.push(tokio::spawn(task2(i)));
        }
        for t in tasks {
            let _ = join!(t);
        }
    }

    //Xcp::get().write_a2l();
    //Xcp::stop_server();
}
