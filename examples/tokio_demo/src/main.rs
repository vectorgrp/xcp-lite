// xcp-lite - tokio_demo
// Visualizes in CANape how tokio starts tasks in its worker threaad pool

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use tokio::{join, time::sleep};

use xcp::*;

// Asynchronous task, measures index, sleeps 100ms, measures -index and ends
// Demonstrates multi instance measurement
// There will be an event and an instance of index for each worker thread tokio uses
#[allow(dead_code)]
async fn task(task_index: u16) {
    let mut index: i16 = task_index as i16;
    trace!("task {} start", index);
    let event = daq_create_event_instance!("task");
    daq_register!(index, event, "Task index", "");
    event.trigger();
    sleep(tokio::time::Duration::from_millis(2)).await;
    index = -index;
    event.trigger();
    trace!("task {} end", index);
}

#[tokio::main]
async fn main() {
    println!("xcp-lite tokio demo");

    env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();

    XcpBuilder::new("tokio_demo").enable_a2l(true).start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1464).unwrap();

    trace!("Start");

    loop {
        sleep(tokio::time::Duration::from_secs(1)).await;

        let mut tasks = Vec::new();

        const N: u16 = 100;
        for i in 1..=N {
            tasks.push(tokio::spawn(task(i)));
        }
        for t in tasks {
            let _ = join!(t);
        }
    }

    //Xcp::get().write_a2l();
    //Xcp::stop_server();
}
