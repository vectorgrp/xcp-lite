// multi_task
// Integration test for XCP in a application with dynamic tasks
// Uses the test XCP client in test_executor

use xcp::*;

mod test_executor;
use test_executor::test_executor;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, thread};
use tokio::time::Duration;

use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// XCP

const OPTION_SERVER_ADDR: [u8; 4] = [127, 0, 0, 1]; // Localhost
const OPTION_SERVER_PORT: u16 = 5555;
const OPTION_TRANSPORT_LAYER: XcpTransportLayer = XcpTransportLayer::Udp; // XcpTransportLayer::TcpIp or XcpTransportLayer::UdpIp
const OPTION_SEGMENT_SIZE: u16 = 1500 - 28; // UDP MTU
const OPTION_LOG_LEVEL: XcpLogLevel = XcpLogLevel::Info; // log::LevelFilter::Off, Error=1, Warn=2, Info=3, Debug=4, Trace=5

//-----------------------------------------------------------------------------
// Calibration Segment

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage1 {
    run: bool,
    counter_max: u32,
    cycle_time_us: u32,
}

const CAL_PAR1: CalPage1 = CalPage1 {
    run: true,
    counter_max: 10,
    cycle_time_us: 1000,
};

//-----------------------------------------------------------------------------

fn task1(cal_seg: CalSeg<CalPage1>) {
    let mut event = daq_create_event!("task", 8);

    let mut counter: u32 = 0;
    let mut counter_max: u32 = 0;

    loop {
        thread::sleep(Duration::from_micros(cal_seg.cycle_time_us as u64));

        daq_capture!(counter_max, event);
        daq_capture!(counter, event);
        event.trigger();

        counter += 1;
        counter_max = cal_seg.counter_max;
        if counter > counter_max {
            break;
        }
    }

    debug!("Task1 terminated");
}

// Test task will be instantiated multiple times
fn task0(cal_seg: CalSeg<CalPage1>) {
    loop {
        thread::sleep(Duration::from_micros(100));

        let c = cal_seg.clone();
        let t = thread::spawn(move || {
            task1(c);
        });
        t.join().unwrap();

        cal_seg.sync();

        // Check for termination
        if !cal_seg.run {
            break;
        }
    }

    info!("Task0 terminated");
}

//-----------------------------------------------------------------------------
// Integration test single threads calibration

#[tokio::test]
async fn test_multi_task() {
    env_logger::Builder::new()
        .filter_level(OPTION_LOG_LEVEL.to_log_level_filter())
        .init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L writer
    match XcpBuilder::new("xcp_lite")
        .set_log_level(OPTION_LOG_LEVEL)
        .enable_a2l(true)
        .set_epk("EPK_TEST")
        .start_server(
            OPTION_TRANSPORT_LAYER,
            OPTION_SERVER_ADDR,
            OPTION_SERVER_PORT,
            OPTION_SEGMENT_SIZE,
        ) {
        Err(res) => {
            error!("XCP initialization failed: {:?}", res);
            return;
        }
        Ok(xcp) => xcp,
    };

    // Create a calibration segment
    let cal_seg = Xcp::create_calseg("cal_seg", &CAL_PAR1, true);

    // Start the test task
    let t0 = thread::spawn(move || {
        task0(cal_seg);
    });

    // Execute the test
    test_executor(false, false, OPTION_LOG_LEVEL).await; // Start the test executor XCP client

    t0.join().unwrap();

    Xcp::stop_server();
}
