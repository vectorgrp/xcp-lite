// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in xcp_client

// cargo test --features=json --features=auto_reg -- --test-threads=1 --nocapture  --test test_multi_thread

#![allow(unused_assignments)]

use xcp::*;
use xcp_type_description::prelude::*;

mod xcp_test_executor;
use xcp_test_executor::xcp_test_executor;
use xcp_test_executor::MULTI_THREAD_TASK_COUNT;
use xcp_test_executor::OPTION_LOG_LEVEL;
use xcp_test_executor::OPTION_XCP_LOG_LEVEL;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use serde::{Deserialize, Serialize};
use std::{fmt::Debug, thread};
use tokio::time::Duration;

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
    run: true,             // Stop test task when false
    cycle_time_us: 100000, // Default cycle time 100ms, will be set by xcp_test_executor
    counter_max: 0xFFFF,
    cal_test: 0x5555555500000000u64,
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

// Test task will be instantiated multiple times
fn task(index: usize, cal_seg: CalSeg<CalPage1>) {
    // Measurement variables 112 bytes
    let mut counter: u32 = 0;
    let mut loop_counter: u64 = 0;
    let mut changes: u64 = 0;
    let mut cal_test: u64 = 0;
    let mut counter_max: u32 = 0;
    let mut test0: u64 = 0;
    let test1: u64 = 0;
    let test2: u64 = 0;
    let test3: u64 = 0;
    let test4: u64 = 0;
    let test5: u64 = 0;
    let test6: u64 = 0;
    let test7: u64 = 0;
    let test8: u64 = 0;
    let test9: u64 = 0;
    let test10: u64 = 0;
    let test11: u64 = 0;
    let test12: u64 = 0;
    let test13: u64 = 0;
    let test14: u64 = 0;
    let test15: u64 = 0;
    let test16: u64 = 0;
    let test17: u64 = 0;
    let test18: u64 = 0;
    let test19: u64 = 0;
    let test20: u64 = 0;
    let test21: u64 = 0;
    let test22: u64 = 0;
    let test23: u64 = 0;
    let test24: u64 = 0;
    let test25: u64 = 0;
    let test26: u64 = 0;
    let test27: u64 = 0;
    let test28: u64 = 0;
    let test29: u64 = 0;
    let test30: u64 = 0;
    let test31: u64 = 0;
    let test32: u64 = 0;
    let test33: u64 = 0;
    let test34: u64 = 0;
    let test35: u64 = 0;
    let test36: u64 = 0;
    let test37: u64 = 0;
    let test38: u64 = 0;
    let test39: u64 = 0;

    if index == 0 || index == MULTI_THREAD_TASK_COUNT - 1 {
        info!("Task {} started, initial cycle time = {}us ", index, cal_seg.cycle_time_us);
    } else if index == 1 {
        info!("...");
    }
    let mut cycle_time = cal_seg.cycle_time_us as u64;

    // Create a measurement event instance for this task instance
    // Capture buffer is 16 bytes, to test both modes, direct and buffer measurement
    let mut event = daq_create_event_instance!("task", 16);

    // Measure some variables directly from stack, without using the event capture buffer
    daq_register_instance!(changes, event);
    daq_register_instance!(loop_counter, event);
    daq_register_instance!(counter_max, event);
    daq_register_instance!(counter, event);
    daq_register_instance!(test0, event);
    daq_register_instance!(test1, event);
    daq_register_instance!(test2, event);
    daq_register_instance!(test3, event);
    daq_register_instance!(test4, event);
    daq_register_instance!(test5, event);
    daq_register_instance!(test6, event);
    daq_register_instance!(test7, event);
    daq_register_instance!(test8, event);
    daq_register_instance!(test9, event);
    daq_register_instance!(test10, event);
    daq_register_instance!(test11, event);
    daq_register_instance!(test12, event);
    daq_register_instance!(test13, event);
    daq_register_instance!(test14, event);
    daq_register_instance!(test15, event);
    daq_register_instance!(test16, event);
    daq_register_instance!(test17, event);
    daq_register_instance!(test18, event);
    daq_register_instance!(test19, event);
    daq_register_instance!(test20, event);
    daq_register_instance!(test21, event);
    daq_register_instance!(test22, event);
    daq_register_instance!(test23, event);
    daq_register_instance!(test24, event);
    daq_register_instance!(test25, event);
    daq_register_instance!(test26, event);
    daq_register_instance!(test27, event);
    daq_register_instance!(test28, event);
    daq_register_instance!(test29, event);
    daq_register_instance!(test30, event);
    daq_register_instance!(test31, event);
    daq_register_instance!(test32, event);
    daq_register_instance!(test33, event);
    daq_register_instance!(test34, event);
    daq_register_instance!(test35, event);
    daq_register_instance!(test36, event);
    daq_register_instance!(test37, event);
    daq_register_instance!(test38, event);
    daq_register_instance!(test39, event);

    loop {
        // Sleep for a calibratable amount of time
        let ct = cal_seg.cycle_time_us as u64;
        thread::sleep(Duration::from_micros(ct));
        if cycle_time != ct {
            if index == 0 || index == MULTI_THREAD_TASK_COUNT - 1 {
                info!("Task {} cycle time changed from {}us to {}us", index, cycle_time, ct);
            } else if index == 1 {
                info!("...");
            }
            cycle_time = ct;
        }

        // Modify measurement variables on stack
        loop_counter += 1;
        test0 = loop_counter;

        // Calculate a counter wrapping at cal_seg.counter_max
        counter_max = cal_seg.counter_max;
        counter += 1;
        if counter > counter_max {
            counter = 0;
        }

        // Test calibration - check cal_seg.cal_test is valid and report the number of changes
        if cal_test != cal_seg.cal_test {
            changes += 1;
            cal_test = cal_seg.cal_test;
            assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
        }

        // Capture variable cal_test, to test capture buffer measurement mode
        daq_capture_instance!(cal_test, event);

        // Trigger the measurement event for this task instance
        event.trigger();

        // Synchronize the calibration segment
        cal_seg.sync();

        // Check for termination and check server is healthy
        if loop_counter % 16 == 0 {
            // Check for termination
            if !cal_seg.run {
                break;
            }
            // Server ok ?
            if !Xcp::get().check_server() {
                panic!("XCP server shutdown!");
            }
        }
    }

    if index == 0 || index == MULTI_THREAD_TASK_COUNT - 1 {
        info!("Task {} terminated, loop counter = {}, {} calibration changes observed", index, loop_counter, changes);
    } else if index == 1 {
        info!("...");
    }
    if changes == 0 {
        warn!("Task {} - No calibration changes observed !!!", index);
    }
}

//-----------------------------------------------------------------------------
// Integration test multi thread measurememt and calibration

#[tokio::test]
async fn test_multi_thread() {
    env_logger::Builder::new().filter_level(OPTION_LOG_LEVEL.to_log_level_filter()).init();

    // Initialize XCP driver singleton, the transport layer server and enable the A2L writer
    let xcp = match XcpBuilder::new("test_multi_thread")
        .set_log_level(OPTION_XCP_LOG_LEVEL)
        .set_epk("EPK_TEST")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
    {
        Err(res) => {
            error!("XCP initialization failed: {:?}", res);
            return;
        }
        Ok(xcp) => xcp,
    };

    // Create a calibration segment
    let cal_seg = xcp.create_calseg("cal_seg", &CAL_PAR1, true);

    // Create MULTI_THREAD_TASK_COUNT test tasks
    let mut v = Vec::new();
    for i in 0..MULTI_THREAD_TASK_COUNT {
        let cal_seg = CalSeg::clone(&cal_seg);
        let t = thread::spawn(move || {
            task(i, cal_seg);
        });
        v.push(t);
    }

    thread::sleep(Duration::from_millis(250)); // Wait to give all threads a chance to initialize and enter their loop
    xcp_test_executor(xcp, xcp_test_executor::TestMode::MultiThreadDAQ, "test_multi_thread.a2l", false).await; // Start the test executor XCP client

    info!("Test done. Waiting for tasks to terminate");
    for t in v {
        t.join().ok();
    }

    info!("Stop XCP server");
    xcp.stop_server();
}
