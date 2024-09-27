// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in test_executor

// cargo test --features=json --features=auto_reg -- --test-threads=1 --nocapture  --test test_multi_thread

use xcp::*;
use xcp_type_description::prelude::*;

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
    cycle_time_us: 100000, // Default cycle time 100ms, will be set by test_executor
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
    // Measurement variables
    let mut counter: u32 = 0;
    let mut loop_counter: u64 = 0;
    let mut changes: u64 = 0;
    let mut cal_test: u64 = 0;
    let mut counter_max: u32 = 0;
    let mut test1: u64 = 0;
    let mut test2: u64 = 0;
    let mut test3: u64 = 0;
    let mut test4: u64 = 0;

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
    daq_register_instance!(test1, event);
    daq_register_instance!(test2, event);
    daq_register_instance!(test3, event);
    daq_register_instance!(test4, event);

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
        test1 = loop_counter;
        test2 = loop_counter;
        test3 = loop_counter;
        test4 = loop_counter;
        _ = test1;
        _ = test2;
        _ = test3;
        _ = test4;

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
    test_executor(xcp, test_executor::TestMode::MultiThreadDAQ, "test_multi_thread.a2l", true).await; // Start the test executor XCP client

    info!("Test done. Waiting for tasks to terminate");
    for t in v {
        t.join().ok();
    }

    info!("Stop XCP server");
    xcp.stop_server();
}
