// single_thread
// Integration test for XCP in a single thread application
// Uses the test XCP client in xcp_client

// cargo test -- --test-threads=1 --features=serde --nocapture  --test test_tokio_single_thread

use xcp::*;
use xcp_type_description::prelude::*;

mod xcp_test_executor;
use xcp_test_executor::xcp_test_executor;
use xcp_test_executor::OPTION_LOG_LEVEL;
use xcp_test_executor::OPTION_XCP_LOG_LEVEL;

mod xcp_server_task;

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

// Test task will be instanciated only once
fn task(cal_seg: CalSeg<CalPage1>) {
    let mut loop_counter: u32 = 0;
    let mut changes: u32 = 0;
    let mut cal_test: u64 = 0;
    let mut counter_max: u32 = cal_seg.counter_max;
    let mut counter: u32 = 0;

    // Create a DAQ event and register local variables for measurment
    let event = daq_create_event!("task");
    daq_register!(changes, event);
    daq_register!(loop_counter, event);
    daq_register!(counter_max, event);
    daq_register!(counter, event);
    daq_register!(cal_test, event);

    loop {
        // Sleep for a calibratable amount of microseconds
        thread::sleep(Duration::from_micros(cal_seg.cycle_time_us as u64));
        loop_counter += 1;

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

        // Test calibration data validity
        if cal_test != cal_seg.cal_test {
            changes += 1;
            cal_test = cal_seg.cal_test;
            assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
        }

        // Trigger DAQ event
        // daq_capture!(cal_test, event);
        // daq_capture!(counter_max, event);
        // daq_capture!(counter, event);
        event.trigger();

        // Synchronize the calibration segment
        cal_seg.sync();

        // Check for termination
        if !cal_seg.run {
            break;
        }
    }

    debug!("Task terminated, loop counter = {}, {} changes observed", loop_counter, changes);
}

//-----------------------------------------------------------------------------
// Integration test single thread measurement and calibration

#[ignore]
#[tokio::test]
async fn test_tokio_single_thread() {
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(OPTION_LOG_LEVEL.to_log_level_filter())
        .try_init()
        .ok();

    info!("Running test_tokio_single_thread");

    // Start tokio XCP server
    // Initialize the xcplib transport and protocol layer only, not the server
    let xcp: &'static Xcp = XcpBuilder::new("test_tokio_single_thread")
        .set_log_level(OPTION_XCP_LOG_LEVEL)
        .set_epk("EPK_TEST")
        .tl_start()
        .map_err(|e| error!("{}", e))
        .unwrap();
    let _xcp_task = tokio::spawn(xcp_server_task::xcp_task(xcp, [127, 0, 0, 1], 5555));

    // Create a calibration segment
    let cal_seg = xcp.create_calseg("cal_seg", &CAL_PAR1);
    cal_seg.register_fields();

    // Create a test task
    let t1 = thread::spawn(move || {
        task(cal_seg);
    });

    xcp_test_executor(
        xcp,
        xcp_test_executor::TestModeCal::Cal,
        xcp_test_executor::TestModeDaq::SingleThreadDAQ,
        "test_tokio_single_thread.a2l",
        false,
    )
    .await; // Start the test executor XCP client

    t1.join().unwrap();

    let _ = std::fs::remove_file("test_tokio_single_thread.a2l");
}
