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
// Test settings

const TEST_CAL: xcp_test_executor::TestModeCal = xcp_test_executor::TestModeCal::Cal; // Execute calibration tests: Cal or None

const TEST_DAQ: xcp_test_executor::TestModeDaq = xcp_test_executor::TestModeDaq::MultiThreadDAQ; // Execute measurement tests: MultiThreadDAQ or None

const TEST_UPLOAD_A2L: bool = true; // Upload A2L file

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
    let mut test1: u64 = 0;
    let mut test2: u64 = 0;
    let mut test3: u64 = 0;
    let mut test4: u64 = 0;
    let mut test5: u64 = 0;
    let mut test6: u64 = 0;
    let mut test7: u64 = 0;
    let mut test8: u64 = 0;
    let mut test9: u64 = 0;
    let mut test10: u64 = 0;
    let mut test11: u64 = 0;
    let mut test12: u64 = 0;
    let mut test13: u64 = 0;
    let mut test14: u64 = 0;
    let mut test15: u64 = 0;
    let mut test16: u64 = 0;
    let mut test17: u64 = 0;
    let mut test18: u64 = 0;
    let mut test19: u64 = 0;
    let mut test20: u64 = 0;
    let mut test21: u64 = 0;
    let mut test22: u64 = 0;
    let mut test23: u64 = 0;
    let mut test24: u64 = 0;
    let mut test25: u64 = 0;
    let mut test26: u64 = 0;
    let mut test27: u64 = 0;
    let mut test28: u64 = 0;
    let mut test29: u64 = 0;
    let mut test30: u64 = 0;
    let mut test31: u64 = 0;
    let mut test32: u64 = 0;
    let mut test33: u64 = 0;
    let mut test34: u64 = 0;
    let mut test35: u64 = 0;
    let mut test36: u64 = 0;
    let mut test37: u64 = 0;
    let mut test38: u64 = 0;
    let mut test39: u64 = 0;
    let mut test40: u64 = 0;
    let mut test41: u64 = 0;
    let mut test42: u64 = 0;
    let mut test43: u64 = 0;
    let mut test44: u64 = 0;
    let mut test45: u64 = 0;
    let mut test46: u64 = 0;
    let mut test47: u64 = 0;
    let mut test48: u64 = 0;
    let mut test49: u64 = 0;
    let mut test50: u64 = 0;
    let mut test51: u64 = 0;
    let mut test52: u64 = 0;
    let mut test53: u64 = 0;
    let mut test54: u64 = 0;
    let mut test55: u64 = 0;
    let mut test56: u64 = 0;
    let mut test57: u64 = 0;
    let mut test58: u64 = 0;
    let mut test59: u64 = 0;
    let mut test60: u64 = 0;
    let mut test61: u64 = 0;
    let mut test62: u64 = 0;
    let mut test63: u64 = 0;

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
        test0 = loop_counter + 1;
        test1 = test0 + 1;
        test2 = test1 + 1;
        test3 = test2 + 1;
        test4 = test3 + 1;
        test5 = test4 + 1;
        test6 = test5 + 1;
        test7 = test6 + 1;
        test8 = test7 + 1;
        test9 = test8 + 1;
        test10 = test9 + 1;
        test11 = test10 + 1;
        test12 = test11 + 1;
        test13 = test12 + 1;
        test14 = test13 + 1;
        test15 = test14 + 1;
        test16 = test15 + 1;
        test17 = test16 + 1;
        test18 = test17 + 1;
        test19 = test18 + 1;
        test20 = test19 + 1;
        test21 = test20 + 1;
        test22 = test21 + 1;
        test23 = test22 + 1;
        test24 = test23 + 1;
        test25 = test24 + 1;
        test26 = test25 + 1;
        test27 = test26 + 1;
        test28 = test27 + 1;
        test29 = test28 + 1;
        test30 = test29 + 1;
        test31 = test30 + 1;
        test32 = test31 + 1;
        test33 = test32 + 1;
        test34 = test33 + 1;
        test35 = test34 + 1;
        test36 = test35 + 1;
        test37 = test36 + 1;
        test38 = test37 + 1;
        test39 = test38 + 1;
        test40 = test39 + 1;
        test41 = test40 + 1;
        test42 = test41 + 1;
        test43 = test42 + 1;
        test44 = test43 + 1;
        test45 = test44 + 1;
        test46 = test45 + 1;
        test47 = test46 + 1;
        test48 = test47 + 1;
        test49 = test48 + 1;
        test50 = test49 + 1;
        test51 = test50 + 1;
        test52 = test51 + 1;
        test53 = test52 + 1;
        test54 = test53 + 1;
        test55 = test54 + 1;
        test56 = test55 + 1;
        test57 = test56 + 1;
        test58 = test57 + 1;
        test59 = test58 + 1;
        test60 = test59 + 1;
        test61 = test50 + 1;
        test61 = test60 + 1;
        test62 = test61 + 1;
        test63 = test62 + 1;
        _ = test63;

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
    xcp_test_executor(xcp, TEST_CAL, TEST_DAQ, "test_multi_thread.a2l", TEST_UPLOAD_A2L).await; // Start the test executor XCP client

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
