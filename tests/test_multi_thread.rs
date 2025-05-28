// test_multi_thread
// Integration test for XCP in a multi threaded application
// Uses the test XCP client in xcp_client

// cargo test --features=a2l_reader -- --test-threads=1 --nocapture  --test test_multi_thread

#![allow(unused_assignments)]
#![allow(unused_imports)]

use log::{debug, error, info, trace, warn};
use std::{fmt::Debug, thread};
use tokio::time::Duration;

use xcp_lite::registry::*;
use xcp_lite::*;

mod xcp_test_executor;
use xcp_test_executor::OPTION_LOG_LEVEL;
use xcp_test_executor::OPTION_XCP_LOG_LEVEL;
use xcp_test_executor::test_executor;

//-----------------------------------------------------------------------------
// Test settings

const TEST_CAL: xcp_test_executor::TestModeCal = xcp_test_executor::TestModeCal::Cal; // Execute calibration tests: Cal or None
const TEST_DAQ: xcp_test_executor::TestModeDaq = xcp_test_executor::TestModeDaq::DaqMultiThread; // Execute measurement tests: MultiThreadDAQ or None
const TEST_TASK_COUNT: usize = 16; // Number of test tasks to create
const TEST_SIGNAL_COUNT: usize = 10; // Number of signals is TEST_SIGNAL_COUNT + 5 for each task
const TEST_DURATION_MS: u64 = 5000;
const TEST_CYCLE_TIME_US: u32 = 1000; // Cycle time in microseconds

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
    run: true,                         // Stop test task when false
    cycle_time_us: TEST_CYCLE_TIME_US, // Cycle time in microseconds, // Default 1ms, will be set by test executor
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

// Create a static cell for the calibration segment, which is shared between the threads
// The alternative would be to move a clone of a CalSeg into each thread
static CAL_SEG1: std::sync::OnceLock<CalCell<CalPage1>> = std::sync::OnceLock::new();

//-----------------------------------------------------------------------------

// Test task will be instantiated multiple times
fn task(index: usize) {
    let cal_seg = CAL_SEG1.get().unwrap().clone_calseg();

    if index == 0 || index == TEST_TASK_COUNT - 1 {
        info!("Task {} started", index);
    } else if index == 1 {
        info!("...");
    }

    let mut cal_test: u64 = 0;
    let mut counter: u32 = 0;
    let mut loop_counter: u64 = 0;
    let mut changes: u64 = 0;
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

    let mut event = daq_create_event_tli!("task", 16);
    daq_register_tli!(counter, event);
    daq_register_tli!(loop_counter, event);
    daq_register_tli!(changes, event);
    daq_register_tli!(counter_max, event);
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

    loop {
        let cal_seg = cal_seg.read_lock();

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
        assert_eq!(cal_seg.sync_test1, cal_seg.sync_test2);

        // Capture variable cal_test, to test capture buffer measurement mode
        daq_capture_tli!(cal_test, event);

        // Trigger the measurement event for this task instance
        event.trigger();

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

    if index == 0 || index == TEST_TASK_COUNT - 1 {
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

    // Initialize XCP server
    let xcp = match Xcp::get()
        .set_app_name("test_multi_thread")
        .set_app_revision("EPK1.0.0")
        .set_log_level(OPTION_XCP_LOG_LEVEL)
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1024 * 256)
    {
        Err(res) => {
            error!("XCP initialization failed: {:?}", res);
            return;
        }
        Ok(xcp) => xcp,
    };

    // Create a static calibration segment shared between the threads
    let cal_seg = CAL_SEG1.get_or_init(|| CalCell::new("cal_seg", &CAL_PAR1)).clone_calseg();
    cal_seg.register_fields(); // Register all struct fields (with meta data from annotations) in the A2L registry

    // Create TEST_TASK_COUNT test tasks
    let mut v = Vec::new();
    for i in 0..TEST_TASK_COUNT {
        let t = thread::spawn(move || {
            task(i);
        });
        v.push(t);
    }

    // In shm_mode, registry has to be finilized manually
    thread::sleep(Duration::from_micros(100000));
    xcp.finalize_registry().unwrap(); // Write the A2L file

    thread::sleep(Duration::from_millis(250)); // Wait to give all threads a chance to initialize and enter their loop
    test_executor(TEST_CAL, TEST_DAQ, TEST_DURATION_MS, TEST_TASK_COUNT, TEST_SIGNAL_COUNT, TEST_CYCLE_TIME_US as u64).await; // Start the test executor XCP client

    debug!("Test done. Waiting for tasks to terminate");
    for t in v {
        t.join().unwrap();
    }

    // Stop and shutdown the XCP server
    debug!("Stop XCP server");
    xcp.stop_server();
    info!("Server stopped");

    let _ = std::fs::remove_file("test_multi_thread.a2l");
}
