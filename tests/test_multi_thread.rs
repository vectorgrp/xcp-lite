// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in xcp_client

// cargo test --features=a2l_reader --features=serde -- --test-threads=1 --nocapture  --test test_multi_thread

#![allow(unused_assignments)]

use xcp::*;

mod xcp_test_executor;
use xcp_test_executor::xcp_test_executor;
use xcp_test_executor::MULTI_THREAD_TASK_COUNT;
use xcp_test_executor::OPTION_LOG_LEVEL;
use xcp_test_executor::OPTION_XCP_LOG_LEVEL;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use std::{fmt::Debug, thread};
use tokio::time::Duration;

//-----------------------------------------------------------------------------
// Calibration Segment

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    run: bool,
    counter_max: u32,
    cal_test: u64,
    sync_test1: u16,
    sync_test2: u16,
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
    sync_test1: 0,
    sync_test2: 0,
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
    let test40: u64 = 0;
    let test41: u64 = 0;
    let test42: u64 = 0;
    let test43: u64 = 0;
    let test44: u64 = 0;
    let test45: u64 = 0;
    let test46: u64 = 0;
    let test47: u64 = 0;
    let test48: u64 = 0;
    let test49: u64 = 0;
    let test50: u64 = 0;
    let test51: u64 = 0;
    let test52: u64 = 0;
    let test53: u64 = 0;
    let test54: u64 = 0;
    let test55: u64 = 0;
    let test56: u64 = 0;
    let test57: u64 = 0;
    let test58: u64 = 0;
    let test59: u64 = 0;
    let test60: u64 = 0;
    let test61: u64 = 0;
    let test62: u64 = 0;
    let test63: u64 = 0;

    if index == 0 || index == MULTI_THREAD_TASK_COUNT - 1 {
        info!("Task {} started, initial cycle time = {}us ", index, cal_seg.cycle_time_us);
    } else if index == 1 {
        info!("...");
    }

    // Create a measurement event instance for this task instance
    // Capture buffer is 16 bytes, to test both modes, direct and buffer measurement
    let mut event = daq_create_event_tli!("task", 16);

    // Measure some variables directly from stack, without using the event capture buffer
    daq_register_tli!(changes, event);
    daq_register_tli!(loop_counter, event);
    daq_register_tli!(counter_max, event);
    daq_register_tli!(counter, event);
    //daq_register_tli!(cal_test, event);

    daq_register_tli!(test0, event);
    daq_register_tli!(test1, event);
    daq_register_tli!(test2, event);
    daq_register_tli!(test3, event);
    daq_register_tli!(test4, event);
    daq_register_tli!(test5, event);
    daq_register_tli!(test6, event);
    daq_register_tli!(test7, event);
    daq_register_tli!(test8, event);
    daq_register_tli!(test9, event);
    daq_register_tli!(test10, event);
    daq_register_tli!(test11, event);
    daq_register_tli!(test12, event);
    daq_register_tli!(test13, event);
    daq_register_tli!(test14, event);
    daq_register_tli!(test15, event);
    daq_register_tli!(test16, event);
    daq_register_tli!(test17, event);
    daq_register_tli!(test18, event);
    daq_register_tli!(test19, event);
    daq_register_tli!(test20, event);
    daq_register_tli!(test21, event);
    daq_register_tli!(test22, event);
    daq_register_tli!(test23, event);
    daq_register_tli!(test24, event);
    daq_register_tli!(test25, event);
    daq_register_tli!(test26, event);
    daq_register_tli!(test27, event);
    daq_register_tli!(test28, event);
    daq_register_tli!(test29, event);
    daq_register_tli!(test30, event);
    daq_register_tli!(test31, event);
    daq_register_tli!(test32, event);
    daq_register_tli!(test33, event);
    daq_register_tli!(test34, event);
    daq_register_tli!(test35, event);
    daq_register_tli!(test36, event);
    daq_register_tli!(test37, event);
    daq_register_tli!(test38, event);
    daq_register_tli!(test39, event);
    daq_register_tli!(test40, event);
    daq_register_tli!(test41, event);
    daq_register_tli!(test42, event);
    daq_register_tli!(test43, event);
    daq_register_tli!(test44, event);
    daq_register_tli!(test45, event);
    daq_register_tli!(test46, event);
    daq_register_tli!(test47, event);
    daq_register_tli!(test48, event);
    daq_register_tli!(test49, event);
    daq_register_tli!(test50, event);
    daq_register_tli!(test51, event);
    daq_register_tli!(test52, event);
    daq_register_tli!(test53, event);
    daq_register_tli!(test54, event);
    daq_register_tli!(test55, event);
    daq_register_tli!(test56, event);
    daq_register_tli!(test57, event);
    daq_register_tli!(test58, event);
    daq_register_tli!(test59, event);
    daq_register_tli!(test60, event);
    daq_register_tli!(test61, event);
    daq_register_tli!(test62, event);
    daq_register_tli!(test63, event);

    loop {
        // Sleep for a calibratable amount of time
        thread::sleep(Duration::from_micros(cal_seg.cycle_time_us as u64));

        // Modify measurement variables on stack
        loop_counter += 1;
        test0 = loop_counter;
        _ = test0;

        // Calculate a counter wrapping at cal_seg.counter_max
        counter_max = cal_seg.counter_max;
        counter += 1;
        if counter > counter_max {
            counter = 0;
        }

        // Test atomic calibration
        // Check that modified cal_seg.cal_test value is not corrupted and report the number of changes
        if cal_test != cal_seg.cal_test {
            changes += 1;
            cal_test = cal_seg.cal_test;
            assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
        }

        // Test consistent calibration
        {
            // Syncronize the calibration segment and get a read lock
            // Check that modified values of sync_test1/2 are always equal
            let cal_seg = cal_seg.read_lock();
            assert_eq!(cal_seg.sync_test1, cal_seg.sync_test2);
        }

        // Capture variable cal_test, to test capture buffer measurement mode
        daq_capture_tli!(cal_test, event);

        // Trigger the measurement event for this task instance
        event.trigger();

        // Synchronize the calibration segment
        cal_seg.sync();

        // Check for termination and check server is healthy
        if loop_counter % 256 == 0 {
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
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .filter_level(OPTION_LOG_LEVEL)
        .init();

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
    let cal_seg = xcp.create_calseg("cal_seg", &CAL_PAR1);
    cal_seg.register_fields();

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
    xcp_test_executor(
        xcp,
        xcp_test_executor::TestModeCal::Cal,
        xcp_test_executor::TestModeDaq::MultiThreadDAQ,
        "test_multi_thread.a2l",
        true,
    )
    .await; // Start the test executor XCP client

    info!("Test done. Waiting for tasks to terminate");
    for t in v {
        t.join().unwrap();
    }

    // Stop and shutdown the XCP server
    info!("Stop XCP server");
    xcp.stop_server();
    info!("Server stopped");

    let _ = std::fs::remove_file("test_multi_thread.a2l");
}
