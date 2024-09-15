// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in test_executor

// cargo test --features=json --features=auto_reg -- --test-threads=1 --nocapture  --test test_tokio_multi_thread

use xcp::*;
use xcp_type_description::prelude::*;

mod xcp_server;

mod test_executor;
use test_executor::test_executor;
use test_executor::MULTI_THREAD_TASK_COUNT;
use test_executor::OPTION_LOG_LEVEL;
use test_executor::OPTION_XCP_LOG_LEVEL;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, thread};
use tokio::time::Duration;

//-----------------------------------------------------------------------------
// Logging

// const OPTION_LOG_LEVEL: XcpLogLevel = XcpLogLevel::Info;
// const OPTION_XCP_LOG_LEVEL: XcpLogLevel = XcpLogLevel::Info;

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
fn task(index: usize, cal_seg: CalSeg<CalPage1>) {
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

    let mut event_time: u64 = 0;
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

        daq_capture_instance!(cal_test, event);

        let start_time = std::time::Instant::now();
        event.trigger();
        let elapsed = start_time.elapsed();
        event_time += elapsed.as_nanos() as u64;

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
        }
    }

    if index == 0 {
        info!(
            "Task {} loop counter = {}, {} changes observed, {}ns per event",
            index,
            loop_counter,
            changes,
            event_time / loop_counter
        );
    }
}

//-----------------------------------------------------------------------------
// Integration test multi thread measurememt and calibration

//#[ignore]
#[tokio::test]
async fn test_tokio_multi_thread() {
    env_logger::Builder::new().filter_level(OPTION_LOG_LEVEL.to_log_level_filter()).init();

    // Start tokio XCP server
    // Initialize the xcplib transport and protocol layer only, not the server
    let xcp: &'static Xcp = XcpBuilder::new("test_tokio_multi_thread").set_log_level(OPTION_XCP_LOG_LEVEL).set_epk("EPK_TEST").tl_start().unwrap();
    let _xcp_task = tokio::spawn(xcp_server::xcp_task(xcp, [127, 0, 0, 1], 5555));

    // Create a calibration segment
    let cal_seg = xcp.create_calseg("cal_seg", &CAL_PAR1, true);

    // Create n test tasks
    let mut v = Vec::new();
    for index in 0..MULTI_THREAD_TASK_COUNT {
        let cal_seg = CalSeg::clone(&cal_seg);
        let t = thread::spawn(move || {
            task(index, cal_seg);
        });
        v.push(t);
    }

    test_executor(xcp, test_executor::TestMode::MultiThreadDAQ, "test_tokio_multi_thread.a2l", false).await; // Start the test executor XCP client

    for t in v {
        t.join().ok();
    }
}
