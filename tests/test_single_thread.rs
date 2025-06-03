// single_thread
// Integration test for XCP in a single thread application
// Uses the test XCP client in module xcp_client

// cargo test --features=a2l_reader -- --test-threads=1 --nocapture  --test test_single_thread

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
const TEST_DAQ: xcp_test_executor::TestModeDaq = xcp_test_executor::TestModeDaq::DaqSingleThread; // Execute measurement tests: DaqSingleThread or None
const TEST_DURATION_MS: u64 = 5000;
const TEST_CYCLE_TIME_US: u32 = 1000; // Cycle time in microseconds
const TEST_SIGNAL_COUNT: usize = 10; // Number of signals is TEST_SIGNAL_COUNT + 5 for each task
const TEST_REINIT: bool = true; // Execute reinitialization test

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
    cycle_time_us: TEST_CYCLE_TIME_US,
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

// Test task will be instanciated only once
fn task(cal_seg: CalSeg<CalPage1>) {
    let mut loop_counter: u32 = 0;
    let mut cal_test: u64 = 0;
    let mut changes: u32 = 0;
    let mut counter_max: u32 = cal_seg.read_lock().counter_max;
    let mut counter: u32 = 0;
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

    // Create a DAQ event and register local variables for measurement
    let mut event = daq_create_event!("task", 16);

    daq_register!(loop_counter, event);
    daq_register!(changes, event);
    daq_register!(counter_max, event);
    daq_register!(counter, event);
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
        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(cal_seg.read_lock().cycle_time_us as u64));
        loop_counter += 1;

        {
            let cal_seg = cal_seg.read_lock();
            // Test XCP text messages if counter_max has changed
            if counter_max != cal_seg.counter_max {
                xcp_println!("Task: counter_max calibrated: counter_max={} !!!", cal_seg.counter_max);
            }

            // Create a calibratable wrapping counter signal
            counter_max = cal_seg.counter_max;
            counter += 1;
            if counter > counter_max {
                counter = 0;
            }
            test0 = loop_counter as u64 + 1;
            test1 = test0 + 1;
            test2 = test1 + 1;
            test3 = test2 + 1;
            test4 = test3 + 1;
            test5 = test4 + 1;
            test6 = test5 + 1;
            test7 = test6 + 1;
            test8 = test7 + 1;
            test9 = test8 + 1;
            let _ = test9;

            // Test calibration data validity
            if cal_test != cal_seg.cal_test {
                changes += 1;
                cal_test = cal_seg.cal_test;
                assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
            }
            daq_capture!(cal_test, event);

            // Trigger DAQ event
            // daq_capture!(cal_test, event);
            // daq_capture!(counter_max, event);
            // daq_capture!(counter, event);
            event.trigger();
        }

        // Check for termination
        if !cal_seg.read_lock().run {
            break;
        }

        // Check if the XCP server is still alive
        if loop_counter % 256 == 0 && !Xcp::get().check_server() {
            panic!("XCP server shutdown!");
        }
    }

    debug!("Task terminated, loop counter = {}, {} changes observed", loop_counter, changes);
    Xcp::disconnect_client(Xcp::get());
}

//-----------------------------------------------------------------------------
// Integration test single thread measurement and calibration

#[tokio::test]
async fn test_single_thread() {
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(OPTION_LOG_LEVEL)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .try_init()
        .ok();

    info!("Running test_single_thread");

    // Test calibration and measurement in a single thread

    info!("XCP server initialization 1");
    let _ = std::fs::remove_file("test_single_thread.a2h");

    // Initialize XCP server
    let xcp = match Xcp::get()
        .set_app_name("test_single_thread")
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

    // Create a calibration segment
    let cal_seg = CalSeg::new("cal_seg", &CAL_PAR1);
    cal_seg.register_fields();

    // Create a test task
    let t1 = thread::spawn({
        let cal_seg = cal_seg.clone();
        move || {
            task(cal_seg);
        }
    });

    thread::sleep(Duration::from_micros(100000));
    xcp.finalize_registry().unwrap(); // Write the A2L file

    test_executor(TEST_CAL, TEST_DAQ, TEST_DURATION_MS, 1, TEST_SIGNAL_COUNT, TEST_CYCLE_TIME_US as u64).await; // Start the test executor XCP client

    t1.join().unwrap();
    xcp.stop_server();

    // Reinitialize the XCP server a second time, to check correct shutdown behaviour
    if TEST_REINIT {
        info!("XCP server initialization 2");

        // Initialize the XCPserver, transport layer and protocoll layer a second time
        let xcp = match Xcp::get().start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555, 1024 * 64) {
            Err(res) => {
                error!("XCP initialization failed: {:?}", res);
                return;
            }
            Ok(xcp) => xcp,
        };

        test_executor(
            xcp_test_executor::TestModeCal::None,
            xcp_test_executor::TestModeDaq::None,
            TEST_DURATION_MS,
            1,
            TEST_SIGNAL_COUNT,
            TEST_CYCLE_TIME_US as u64,
        )
        .await; // Start the test executor XCP client

        xcp.stop_server();

        let _ = std::fs::remove_file("test_single_thread.a2l");
    }
}
