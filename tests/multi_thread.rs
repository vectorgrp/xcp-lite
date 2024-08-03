// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in test_executor

use xcp::*;

mod test_executor;
use test_executor::test_executor;
use test_executor::MULTI_THREAD_TASK_COUNT;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, thread};
use tokio::time::Duration;

//-----------------------------------------------------------------------------
// XCP

const OPTION_SERVER_ADDR: [u8; 4] = [127, 0, 0, 1]; // Localhost
const OPTION_SERVER_PORT: u16 = 5555;
const OPTION_TRANSPORT_LAYER: XcpTransportLayer = XcpTransportLayer::Udp; // XcpTransportLayer::TcpIp or XcpTransportLayer::UdpIp
const OPTION_SEGMENT_SIZE: u16 = 1500 - 28; // UDP MTU
const OPTION_LOG_LEVEL: XcpLogLevel = XcpLogLevel::Info;
const OPTION_XCP_LOG_LEVEL: XcpLogLevel = XcpLogLevel::Info;
//-----------------------------------------------------------------------------
// Calibration Segment

use xcp_type_description_derive::XcpTypeDescription;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct TestInts {
    test_bool: bool,
    test_u8: u8,
    test_u16: u16,
    test_u32: u32,
    test_u64: u64,
    test_i8: i8,
    test_i16: i16,
    test_i32: i32,
    test_i64: i64,
    test_f32: f32,
    test_f64: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage1 {
    run: bool,
    counter_max: u32,
    cal_test: u64,
    cycle_time_us: u32,
    page: u8,
    test_ints: TestInts,
}

// Default values for the calibration parameters
const CAL_PAR1: CalPage1 = CalPage1 {
    run: true,
    counter_max: 0xFFFF,
    cal_test: 0x5555555500000000u64,
    cycle_time_us: 1000,
    page: XcpCalPage::Flash as u8,
    test_ints: TestInts {
        test_bool: false,
        test_u8: 0x12,
        test_u16: 0x1234,
        test_u32: 0x12345678,
        test_u64: 0x0102030405060708u64,
        test_i8: -1,
        test_i16: -1,
        test_i32: -1,
        test_i64: -1,
        test_f32: 0.123456E-10,
        test_f64: 0.123456789E-100,
    },
};

//-----------------------------------------------------------------------------

// Test task will be instatiated multiple times
fn task(cal_seg: CalSeg<CalPage1>) {
    let mut counter: u32 = 0;
    let mut loop_counter: u64 = 0;
    let mut changes: u64 = 0;
    let mut cal_test: u64 = 0;
    let mut counter_max: u32 = 0;
    let mut test1: u64 = 0;
    let mut test2: u64 = 0;
    let mut test3: u64 = 0;
    let mut test4: u64 = 0;

    let mut event = daq_create_event_instance!("task");
    daq_register_instance!(changes, event);
    daq_register_instance!(loop_counter, event);
    //daq_register_instance!(cal_test, event); // Measured with capture, pattern checked in DaqDecoder
    daq_register_instance!(counter_max, event);
    daq_register_instance!(counter, event);
    daq_register_instance!(test1, event);
    daq_register_instance!(test2, event);
    daq_register_instance!(test3, event);
    daq_register_instance!(test4, event);

    loop {
        thread::sleep(Duration::from_micros(cal_seg.cycle_time_us as u64)); // Sleep for a calibratable amount of microseconds
        loop_counter += 1;

        // Create a calibratable wrapping counter signal
        counter_max = cal_seg.counter_max;
        counter += 1;
        if counter > counter_max {
            counter = 0;
        }

        // Test calibration data validity
        if cal_test != cal_seg.cal_test {
            changes += 1;
            cal_test = cal_seg.cal_test;
            assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
        }

        // DAQ
        // daq_capture_instance!(changes, event);
        // daq_capture_instance!(loop_counter, event);
        daq_capture_instance!(cal_test, event);
        // daq_capture_instance!(counter_max, event);
        // daq_capture_instance!(counter, event);
        event.trigger();

        // Synchronize the calibration segment
        cal_seg.sync();

        if loop_counter % 256 == 0 {
            test1 = loop_counter;
            test2 = test1 + 1;
            test3 = test2 + 2;
            test4 = test3 + 3;
            _ = test4;

            // Check for termination
            if !cal_seg.run {
                break;
            }

            if !Xcp::check_server() {
                panic!("XCP server shutdown!");
            }
        }
    }

    debug!(
        "Task terminated, loop counter = {}, {} changes observed",
        loop_counter, changes
    );
}

//-----------------------------------------------------------------------------
// Integration test single threads calibration

#[tokio::test]
async fn test_multi_thread() {
    env_logger::Builder::new()
        .filter_level(OPTION_LOG_LEVEL.to_log_level_filter())
        .init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L writer
    match XcpBuilder::new("xcp_lite")
        .set_log_level(OPTION_XCP_LOG_LEVEL)
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

    // Create n test tasks
    let mut v = Vec::new();
    for _ in 0..MULTI_THREAD_TASK_COUNT {
        let cal_seg = CalSeg::clone(&cal_seg);
        let t = thread::spawn(move || {
            task(cal_seg);
        });
        v.push(t);
    }

    test_executor(false, true).await; // Start the test executor XCP client

    for t in v {
        t.join().ok();
    }

    Xcp::stop_server();
    std::fs::remove_file("xcp_client.a2l").ok();
}
