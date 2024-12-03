// multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in xcp_client

// cargo test -- --test-threads=1 --features=serde --nocapture  --test test_tokio_multi_thread

use xcp::*;

mod xcp_test_executor;
use xcp_test_executor::xcp_test_executor;
use xcp_test_executor::MULTI_THREAD_TASK_COUNT;
use xcp_test_executor::OPTION_LOG_LEVEL;
use xcp_test_executor::OPTION_XCP_LOG_LEVEL;

mod xcp_server_task;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use std::fmt::Debug;

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
async fn task(index: usize, cal_seg: CalSeg<CalPage1>) {
    if index == 0 || index == MULTI_THREAD_TASK_COUNT - 1 {
        info!("Task {} started, initial cycle time = {}us ", index, cal_seg.cycle_time_us);
    } else if index == 1 {
        info!("...");
    }

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

    let event = daq_create_event_instance!("task");
    daq_register_instance!(changes, event);
    daq_register_instance!(loop_counter, event);
    daq_register_instance!(counter_max, event);
    daq_register_instance!(counter, event);
    daq_register_instance!(cal_test, event); // pattern checked in DaqDecoder

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
    daq_register_instance!(test40, event);
    daq_register_instance!(test41, event);
    daq_register_instance!(test42, event);
    daq_register_instance!(test43, event);
    daq_register_instance!(test44, event);
    daq_register_instance!(test45, event);
    daq_register_instance!(test46, event);
    daq_register_instance!(test47, event);
    daq_register_instance!(test48, event);
    daq_register_instance!(test49, event);
    daq_register_instance!(test50, event);
    daq_register_instance!(test51, event);
    daq_register_instance!(test52, event);
    daq_register_instance!(test53, event);
    daq_register_instance!(test54, event);
    daq_register_instance!(test55, event);
    daq_register_instance!(test56, event);
    daq_register_instance!(test57, event);
    daq_register_instance!(test58, event);
    daq_register_instance!(test59, event);
    daq_register_instance!(test60, event);
    daq_register_instance!(test61, event);
    daq_register_instance!(test62, event);
    daq_register_instance!(test63, event);

    loop {
        // Sleep for a calibratable amount of time
        tokio::time::sleep(tokio::time::Duration::from_micros(cal_seg.cycle_time_us as u64)).await;

        // Modify measurement variables on stack
        loop_counter += 1;
        test0 = loop_counter;
        _ = test0;

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

        // Trigger the measurement event for this task instance
        event.trigger();

        // Synchronize the calibration segment
        cal_seg.sync();

        // Check for termination
        if !cal_seg.run {
            break;
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

#[ignore]
#[tokio::test]
async fn test_tokio_multi_thread() {
    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(OPTION_LOG_LEVEL).init();

    // Start tokio XCP server
    // Initialize the xcplib transport and protocol layer only, not the server
    let xcp: &'static Xcp = XcpBuilder::new("test_tokio_multi_thread")
        .set_log_level(OPTION_XCP_LOG_LEVEL)
        .set_epk("EPK_TEST")
        .tl_start()
        .unwrap();
    let _xcp_task = tokio::spawn(xcp_server_task::xcp_task(xcp, [127, 0, 0, 1], 5555));

    // Create a calibration segment
    let cal_seg = xcp.create_calseg("cal_seg", &CAL_PAR1);
    cal_seg.register_fields();

    // Create n test tasks
    let mut v = Vec::new();
    for index in 0..MULTI_THREAD_TASK_COUNT {
        let cal_seg = CalSeg::clone(&cal_seg);
        let t = tokio::spawn(task(index, cal_seg));
        v.push(t);
    }

    // Wait for the test tasks to start up
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Start the test executor XCP client
    xcp_test_executor(
        xcp,
        xcp_test_executor::TestModeCal::Cal,
        xcp_test_executor::TestModeDaq::MultiThreadDAQ,
        "test_tokio_multi_thread.a2l",
        false,
    )
    .await;

    for t in v {
        let _ = tokio::join!(t);
    }

    let _ = std::fs::remove_file("test_tokio_multi_thread.a2l");
}
