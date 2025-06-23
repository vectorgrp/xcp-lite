//-----------------------------------------------------------------------------
// Module xcp_test_executor
// Runs various tests against a XCP server on local host UDP port 5555

#![allow(dead_code)]
#![allow(unused_imports)]

use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::num::Wrapping;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use tokio::time::{Duration, Instant};

use xcp_client::xcp_client::*;
use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------

// Logging
pub const OPTION_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Debug;
pub const OPTION_XCP_LOG_LEVEL: u8 = 4;

//------------------------------------------------------------------------
// Test parameters
const CAL_TEST_MAX_ITER: u32 = 4000; // Number of calibrations
const CAL_TEST_TASK_SLEEP_TIME_US: u64 = 100; // Calibration check task cycle time in us

pub const MAX_TASK_COUNT: usize = 255; // Max number of threads 

//------------------------------------------------------------------------
// Test error

pub static DAQ_ERROR: AtomicBool = AtomicBool::new(false);
pub static DAQ_PACKETS_LOST: AtomicU32 = AtomicU32::new(0);
pub static DAQ_COUNTER_ERRORS: AtomicU32 = AtomicU32::new(0);
pub static DAQ_BYTES: AtomicU64 = AtomicU64::new(0);

//------------------------------------------------------------------------
// Handle incoming SERV_TEXT data

#[derive(Debug, Clone, Copy)]
struct ServTextDecoder;

impl ServTextDecoder {
    pub fn new() -> ServTextDecoder {
        ServTextDecoder {}
    }
}

impl XcpTextDecoder for ServTextDecoder {
    // Handle incomming text data from XCP server
    fn decode(&self, data: &[u8]) {
        print!("[SERV_TEXT] ");
        let mut j = 0;
        while j < data.len() {
            if data[j] == 0 {
                break;
            }
            print!("{}", data[j] as char);
            j += 1;
        }
    }
}

//------------------------------------------------------------------------
// Handle incoming DAQ data
// Create some test diagnostic data

#[derive(Debug, Clone, Copy)]
pub struct DaqDecoder {
    pub task_count: usize,
    pub timestamp_resolution: u64,
    pub tot_events: u32,
    pub tot_bytes: u64,
    pub packets_lost: u32,
    pub counter_errors: u32,
    pub daq_max: u16,
    pub odt_max: u8,
    pub daq_timestamp: [u64; MAX_TASK_COUNT],
    pub daq_events: [u32; MAX_TASK_COUNT],
    pub max_counter: [u32; MAX_TASK_COUNT],
    pub last_counter: [u32; MAX_TASK_COUNT],
}

impl DaqDecoder {
    pub fn new(task_count: usize) -> DaqDecoder {
        assert!(task_count <= MAX_TASK_COUNT);
        DaqDecoder {
            task_count,
            timestamp_resolution: 1,
            tot_events: 0,
            tot_bytes: 0,
            packets_lost: 0,
            counter_errors: 0,
            daq_max: 0,
            odt_max: 0,
            daq_timestamp: [0; MAX_TASK_COUNT],
            daq_events: [0; MAX_TASK_COUNT],
            max_counter: [0; MAX_TASK_COUNT],
            last_counter: [0; MAX_TASK_COUNT],
        }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    // Set start time and reset
    fn start(&mut self, _odt_entries: Vec<Vec<OdtEntry>>, timestamp: u64) {
        DAQ_BYTES.store(0, std::sync::atomic::Ordering::Relaxed);
        self.tot_events = 0;
        self.tot_bytes = 0;
        self.packets_lost = 0;
        self.counter_errors = 0;
        self.daq_max = 0;
        self.odt_max = 0;
        for i in 0..MAX_TASK_COUNT {
            self.daq_timestamp[i] = timestamp;
            self.daq_events[i] = 0;
            self.max_counter[i] = 0;
            self.last_counter[i] = 0;
        }
    }

    // Set timestamp resolution
    fn set_daq_properties(&mut self, timestamp_resolution: u64, daq_header_size: u8) {
        self.timestamp_resolution = timestamp_resolution;
        assert_eq!(daq_header_size, 4);
    }

    // Handle incomming DAQ DTOs from XCP server
    fn decode(&mut self, lost: u32, buf: &[u8]) {
        self.tot_bytes += buf.len() as u64;
        DAQ_BYTES.store(self.tot_bytes, std::sync::atomic::Ordering::Relaxed);

        if lost > 0 {
            self.packets_lost += lost;
            DAQ_PACKETS_LOST.store(self.packets_lost, std::sync::atomic::Ordering::Relaxed);
            // warn!("PACKETS_LOST = {}", lost);
        }

        let mut timestamp_raw: u32 = 0;
        let data: &[u8];

        // Decode header and raw timestamp
        let daq = (buf[2] as u16) | ((buf[3] as u16) << 8);
        let odt = buf[0];
        if odt == 0 {
            timestamp_raw = (buf[4] as u32) | ((buf[4 + 1] as u32) << 8) | ((buf[4 + 2] as u32) << 16) | ((buf[4 + 3] as u32) << 24);
            data = &buf[8..];
        } else {
            data = &buf[4..];
        }

        assert!((daq as usize) < self.task_count);
        assert!(odt == 0);
        if daq > self.daq_max {
            self.daq_max = daq;
        }

        // Decode raw timestamp as u64
        // Check declining and stuck timestamps
        if odt == 0 {
            let t_last = self.daq_timestamp[daq as usize];
            let tl = (t_last & 0xFFFFFFFF) as u32;
            let mut th = (t_last >> 32) as u32;
            if timestamp_raw < tl {
                th += 1;
            }
            let t = (timestamp_raw as u64) | ((th as u64) << 32);
            if t < t_last {
                warn!("Timestamp of daq {} declining {} -> {}", daq, t_last, t);
            }
            if t == t_last {
                warn!("Timestamp of daq {} stuck at {}", daq, t);
            }
            self.daq_timestamp[daq as usize] = t;
        }

        // Hardcoded decoding of data (only one ODT)
        assert!(odt == 0);
        if odt == 0 && data.len() >= 8 {
            // Check counter_max (+0) and counter (+4)
            let counter_max = (data[0] as u32) | ((data[1] as u32) << 8) | ((data[2] as u32) << 16) | ((data[3] as u32) << 24);
            let counter = (data[4] as u32) | ((data[5] as u32) << 8) | ((data[6] as u32) << 16) | ((data[7] as u32) << 24);
            if counter > counter_max {
                DAQ_ERROR.store(true, std::sync::atomic::Ordering::Relaxed);
                error!("DAQ_ERROR: counter_max={} < counter={}", counter_max, counter);
            }
            if counter_max >= self.max_counter[daq as usize] {
                self.max_counter[daq as usize] = counter_max;
            }

            // Check each counter is incrementing, usually because of packets lost
            if self.daq_events[daq as usize] != 0 && counter != self.last_counter[daq as usize] + 1 && counter != 0 && daq != 0 {
                let count = DAQ_COUNTER_ERRORS.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                trace!(
                    "DAQ_COUNTER_ERRORS: {} daq={} {} -> {} max={} ",
                    count, daq, self.last_counter[daq as usize], counter, counter_max,
                );
            }
            self.last_counter[daq as usize] = counter;

            // Check cal_test pattern (+8)
            if data.len() >= 16 {
                let cal_test = (data[8] as u64)
                    | ((data[9] as u64) << 8)
                    | ((data[10] as u64) << 16)
                    | ((data[11] as u64) << 24)
                    | ((data[12] as u64) << 32)
                    | ((data[13] as u64) << 40)
                    | ((data[14] as u64) << 48)
                    | ((data[15] as u64) << 56);
                assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
            }

            // Check time (+16)
            if data.len() >= 24 {
                let time = (data[8 + 8] as u64)
                    | ((data[8 + 9] as u64) << 8)
                    | ((data[8 + 10] as u64) << 16)
                    | ((data[8 + 11] as u64) << 24)
                    | ((data[8 + 12] as u64) << 32)
                    | ((data[8 + 13] as u64) << 40)
                    | ((data[8 + 14] as u64) << 48)
                    | ((data[8 + 15] as u64) << 56);

                let cur_time = Xcp::get().get_clock();
                if cur_time < time {
                    error!("Measurement value time is unplausible");
                }
                let delay = cur_time - time;
                if delay > 500000000 {
                    warn!("DAQ event is more than 500ms ({}ms) delayed", delay / 1000000);
                }
            }

            // Check test signals
            if data.len() >= 32 {
                let mut o = 24;
                for i in 0..(data.len() - 24) / 8 {
                    let test = (data[o] as u64)
                        | ((data[o + 1] as u64) << 8)
                        | ((data[o + 2] as u64) << 16)
                        | ((data[o + 3] as u64) << 24)
                        | ((data[o + 4] as u64) << 32)
                        | ((data[o + 5] as u64) << 40)
                        | ((data[o + 6] as u64) << 48)
                        | ((data[o + 7] as u64) << 56);
                    let test_ok = 0x0400_0300_0200_0100 + (0x0001_0001_0001_0001 * i as u64);
                    if test != test_ok {
                        error!("DAQ_ERROR: wrong test signal value test_{} = {:08X}, should be = {:08X}", i, test, test_ok);
                        DAQ_ERROR.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                    o = o + 8;
                }
            }

            trace!(
                "DAQ: daq = {}, odt = {} timestamp = {} counter={}, counter_max={} (rest={:?})",
                daq,
                odt,
                timestamp_raw,
                counter,
                counter_max,
                &data[6..]
            );

            self.daq_events[daq as usize] += 1;
            self.tot_events += 1;
        } // odt==0
    }
}

//-----------------------------------------------------------------------
// Execute tests

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TestModeDaq {
    None,
    DaqSingleThread,
    DaqMultiThread,
    DaqPerformance,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TestModeCal {
    None,
    Cal,
}

// Perform DAQ test
// Returns actual duration and bytes per event
// 0,0 on error
pub async fn test_daq(
    xcp_client: &mut XcpClient,
    test_mode_daq: TestModeDaq,
    test_signal_count: usize,
    daq_test_duration_ms: u64,
    task_count: usize,
    _task_cycle_us: u64,
) -> (bool, u32) {
    let mut error = false;

    // Measurement test loop
    // Create a measurement DAQ list with all instances of measurement counter and counter_max
    // Hard coded order and size in DaqDecoder (counter_max, counter, cal_test, ...)
    if test_mode_daq == TestModeDaq::DaqMultiThread || test_mode_daq == TestModeDaq::DaqPerformance {
        for i in 1..=task_count {
            let counter = "counter_".to_string() + &i.to_string();
            let counter_max = "counter_max_".to_string() + &i.to_string();
            let cal_test = "cal_test_".to_string() + &i.to_string();
            let time = "time_".to_string() + &i.to_string();
            xcp_client.create_measurement_object(counter_max.as_str()).unwrap(); // +0
            xcp_client.create_measurement_object(counter.as_str()).unwrap(); // +4
            xcp_client.create_measurement_object(cal_test.as_str()).unwrap(); // +8
            xcp_client.create_measurement_object(time.as_str()).unwrap(); // +16
            for j in 0..test_signal_count {
                let name = format!("test{}_{}", j, i);
                let res = xcp_client.create_measurement_object(name.as_str());
                if res.is_none() {
                    error!("Test signal not available! Could not create measurement object {}", name);
                    break;
                }
            }
        }
    } else {
        xcp_client.create_measurement_object("counter_max").unwrap(); // +0
        xcp_client.create_measurement_object("counter").unwrap(); // +4
        xcp_client.create_measurement_object("cal_test").unwrap(); // +8
        xcp_client.create_measurement_object("time").unwrap(); // +16
    };
    xcp_client.start_measurement().await.unwrap();

    // Test for given time
    // Every 2ms check if measurement  is still ok
    // Break on error
    let starttime = Instant::now();
    loop {
        if starttime.elapsed().as_millis() > daq_test_duration_ms as u128 {
            break;
        }
        if DAQ_ERROR.load(std::sync::atomic::Ordering::SeqCst) {
            warn!("DAQ error detected, aborting DAQ test loop");
            error = true;
            break;
        }
        let packets_lost = DAQ_PACKETS_LOST.load(std::sync::atomic::Ordering::SeqCst);
        if packets_lost > 0 {
            warn!("DAQ packet loss detected, aborting DAQ test loop");
            break;
        }
        let counter_errors = DAQ_COUNTER_ERRORS.load(std::sync::atomic::Ordering::SeqCst);
        if counter_errors > 0 {
            warn!("DAQ counter error detected, aborting DAQ test loop");
            break;
        }
        tokio::time::sleep(Duration::from_micros(2000)).await;
    }
    let duration_ms = starttime.elapsed().as_millis().try_into().unwrap();

    // Stop DAQ
    let res = xcp_client.stop_measurement().await;
    match res {
        Ok(_) => {
            debug!("DAQ stopped");
        }
        Err(e) => {
            error!("DAQ stop failed: {:?}", e);
            error = true;
        }
    }

    // Wait some time to be sure the queue is emptied
    // The XCP server will not respond to STOP while the queue is not empty
    // But the queue of the client may still contain data or the control channel may need some more time
    tokio::time::sleep(Duration::from_millis(250)).await;

    // Return error state or actual est duration and bytes per event
    if error {
        error!("Error in DAQ test loop after {}ms", duration_ms);
        (false, duration_ms)
    } else {
        (true, duration_ms)
    }
}

// Consistent calibration test loop
// Do MAX_ITER consistent calibrations on cal_seg.sync_test1/2 cal_test
// Thread task will panic if inconsistent
async fn test_consistent_calibration(xcp_client: &mut XcpClient) -> bool {
    let mut error_state = false;

    tokio::time::sleep(Duration::from_micros(10000)).await;

    // Create calibration variables
    let sync_test1 = xcp_client.create_calibration_object("cal_seg.sync_test1").await;
    if sync_test1.is_err() {
        error!("could not create calibration object cal_seg.sync_test1");
        return false;
    }
    let addr_sync_test1 = sync_test1.unwrap().get_a2l_addr(xcp_client);
    let sync_test2 = xcp_client.create_calibration_object("cal_seg.sync_test2").await;
    if sync_test2.is_err() {
        error!("could not create calibration object cal_seg.sync_test2");
        return false;
    }
    let addr_sync_test2 = sync_test2.unwrap().get_a2l_addr(xcp_client);

    info!("start consistent calibration test loop");

    // Calibration loop
    // Set calibration variable cal_test to a defined pattern which will be checked by the server application task
    for i in 0..CAL_TEST_MAX_ITER {
        let value: u16 = (i & 0xFFFF) as u16;

        xcp_client
            .modify_begin()
            .await
            .map_err(|e| {
                error_state = true;
                error!("modify begin: {:?}", e);
            })
            .ok();

        xcp_client // SHORT_DOWNLOAD cal_seg.test_u64
            .short_download(addr_sync_test1.addr, addr_sync_test1.ext, &value.to_le_bytes())
            .await
            .map_err(|e| {
                error_state = true;
                error!("download sync_test1: {:?}", e);
            })
            .ok();

        xcp_client // SHORT_DOWNLOAD cal_seg.test_u64
            .short_download(addr_sync_test2.addr, addr_sync_test2.ext, &value.to_le_bytes())
            .await
            .map_err(|e| {
                error_state = true;
                error!("download sync_test2: {:?}", e);
            })
            .ok();

        xcp_client
            .modify_end()
            .await
            .map_err(|e| {
                error_state = true;
                error!("modify end: {:?}", e);
            })
            .ok();

        if error_state {
            break;
        }
    }

    if error_state {
        error!("Error in calibration test loop");
    } else {
        info!("consistent calibration test loop done, {} iterations", CAL_TEST_MAX_ITER);
    }
    return !error_state;
}

//-----------------------------------------------------------------------

// Calibration test
async fn test_calibration(xcp_client: &mut XcpClient, _task_cycle_us: u64) -> bool {
    let mut error_state = false;

    info!("Start calibration test");

    // Wait some time to be sure the queue is emptied
    // The XCP server should not respond to STOP while the queue is not empty
    // But the queue of the client may still contain data or the control channel may need some time
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test signed
    debug!("Create calibration object cal_seg.test_ints.test_i16");
    let test_i32 = xcp_client
        .create_calibration_object("cal_seg.test_ints.test_i16")
        .await
        .expect("could not create calibration object cal_seg.test_ints.test_i16");
    let v = xcp_client.get_value_i64(test_i32);
    assert_eq!(v, -1);
    xcp_client.set_value_i64(test_i32, 1).await.unwrap();
    let v = xcp_client.get_value_i64(test_i32);
    assert_eq!(v, 1);
    xcp_client.set_value_i64(test_i32, -1).await.unwrap();
    let v = xcp_client.get_value_i64(test_i32);
    assert_eq!(v, -1);

    // Test u64
    debug!("Create calibration object cal_seg.cal_test");
    let cal_test = xcp_client
        .create_calibration_object("cal_seg.cal_test")
        .await
        .expect("could not create calibration object cal_seg.cal_test");
    let v = xcp_client.get_value_u64(cal_test);
    //println!("cal_test={:X}", v);
    assert_eq!(v, 0x5555555500000000u64);
    debug!("Create calibration object cal_seg.test_u64");
    let test_u64 = xcp_client
        .create_calibration_object("cal_seg.test_ints.test_u64")
        .await
        .expect("could not create calibration object cal_seg.test_f64");
    let v = xcp_client.get_value_u64(test_u64);
    //println!("test_u64={:X}", v);
    assert_eq!(v, 0x0102030405060708u64);

    // Test f64
    debug!("Create calibration object cal_seg.test_f64");
    let test_f64 = xcp_client
        .create_calibration_object("cal_seg.test_ints.test_f64")
        .await
        .expect("could not create calibration object cal_seg.test_f64");
    let v = xcp_client.get_value_f64(test_f64);
    assert_eq!(v, 0.123456789E-100);

    // Check page switching
    // Check page is ram
    info!("Check ecu cal page");
    let mut page: u8 = xcp_client.get_ecu_page(0).await.unwrap();
    assert!(page == 0);
    page = xcp_client.get_xcp_page(0).await.unwrap();
    assert!(page == 0);

    // Mark the ram page in variable cal_seg.page
    let mut cal_seg_page = xcp_client
        .create_calibration_object("cal_seg.page")
        .await
        .expect("could not create calibration object page");
    xcp_client // init page variable in ram page of cal_seg
        .set_value_u64(cal_seg_page, 0)
        .await
        .unwrap();
    // Switch to default
    xcp_client.set_ecu_page(1).await.unwrap();
    xcp_client.set_xcp_page(1).await.unwrap();
    tokio::time::sleep(Duration::from_micros(100000)).await;
    // Check if cal_seg.page marker is default
    cal_seg_page = xcp_client
        .create_calibration_object("cal_seg.page")
        .await
        .expect("could not create calibration object page");
    page = xcp_client.get_value_u64(cal_seg_page).try_into().unwrap();
    assert_eq!(page, 1);
    // Check if get cal page returns default
    page = xcp_client.get_xcp_page(0).await.unwrap();
    assert_eq!(page, 1);
    page = xcp_client.get_ecu_page(0).await.unwrap();
    assert_eq!(page, 1);
    // Switch back to ram
    xcp_client.set_xcp_page(0).await.unwrap();
    xcp_client.set_ecu_page(0).await.unwrap();

    // Calibration test loop
    // Do MAX_ITER test calibrations on cal_seg.cal_test, task will panic if cal_seg.test_u64 has not the expected pattern
    {
        tokio::time::sleep(Duration::from_micros(10000)).await;
        info!("start calibration test loop, recalibrate cycle time to 50us for maximum number of calibration checks");

        // Speed up task cycle time to CAL_TEST_TASK_SLEEP_TIME_US, this will set the calseg.sync() rate and pattern checking rate
        // Create calibration object for cycle_time_us
        debug!("Create calibration object cal_seg.cycle_time_us");
        let cycle_time_us = xcp_client
            .create_calibration_object("cal_seg.cycle_time_us")
            .await
            .expect("could not create calibration object cal_seg.cycle_time_us");
        let v = xcp_client.get_value_u64(cycle_time_us);
        info!("cal_seg.cycle_time_us = {}", v);
        xcp_client.set_value_u64(cycle_time_us, CAL_TEST_TASK_SLEEP_TIME_US).await.unwrap();

        // Get address of calibration variable cal_seg.cal_test
        let registry = xcp_client.get_registry();
        let instance_cal_test = registry.instance_list.find_instance("cal_seg.cal_test", McObjectType::Characteristic, None).unwrap();
        let addr_cal_test = instance_cal_test.get_address().get_a2l_addr(registry);
        debug!("Address of cal_seg.cal_test = {}:0x{:X}\n", addr_cal_test.0, addr_cal_test.1);

        // Calibration loop
        // Set calibration variable cal_test to a defined pattern which will be checked by the server application task
        let start_time = Instant::now();
        for i in 0..CAL_TEST_MAX_ITER {
            let cal_test = i as u64 + (((i as u64) << 32) ^ 0x5555555500000000u64); // The server task will check this pattern and panic if it is wrong
            trace!("download cal_seg.cal_test = {:X}", cal_test);
            xcp_client // SHORT_DOWNLOAD cal_seg.test_u64
                .short_download(addr_cal_test.1, addr_cal_test.0, &cal_test.to_le_bytes())
                .await
                .map_err(|e| {
                    error_state = true;
                    error!("download cal_seg.cal_test failed: {:?}", e);
                })
                .ok();
            if error_state {
                break;
            }
        }
        let elapsed_time = start_time.elapsed().as_micros();
        let download_time = elapsed_time as f64 / CAL_TEST_MAX_ITER as f64;
        info!(
            "calibration test loop done, {} iterations, duration={}ms, {}us per download, {:.1} KBytes/s",
            CAL_TEST_MAX_ITER,
            elapsed_time / 1000,
            download_time,
            CAL_TEST_MAX_ITER as f64 * 8000.0 / elapsed_time as f64
        );
        if download_time > 100.0 {
            warn!("Calibration download time ({}us) is too high!", download_time);
        }
    } // Calibration test loop

    return !error_state;
}

//-------------------------------------------------------------------------------------------------------------------------------------
// Setup test
// Connect, upload A2l, check EPK, check id, ...
pub async fn test_setup(task_count: usize, load_a2l: bool, upload_a2l: bool) -> (XcpClient, Arc<parking_lot::lock_api::Mutex<parking_lot::RawMutex, DaqDecoder>>) {
    tokio::time::sleep(Duration::from_millis(500)).await;
    debug!("Test setup");

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Create xcp_client and connect the XCP server
    info!("XCP CONNECT");
    let dest_addr = "127.0.0.1:5555".parse().unwrap();
    let local_addr = "0.0.0.0:0".parse().unwrap();
    info!("  dest_addr: {}", dest_addr);
    info!("  local_addr: {}", local_addr);
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);
    let daq_decoder: Arc<parking_lot::lock_api::Mutex<parking_lot::RawMutex, DaqDecoder>> = Arc::new(Mutex::new(DaqDecoder::new(task_count)));
    let serv_text_decoder = ServTextDecoder::new();
    xcp_client.connect(Arc::clone(&daq_decoder), serv_text_decoder).await.unwrap();
    tokio::time::sleep(Duration::from_micros(10000)).await;

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Check command timeout using a command CC_NOP (non standard) without response
    debug!("Check command timeout handling");
    let res = xcp_client.command(CC_NOP).await; // Check unknown command
    match res {
        Ok(_) => panic!("Should timeout"),
        Err(e) => {
            e.downcast_ref::<XcpClientError>()
                .map(|e| {
                    debug!("XCP error code ERROR_CMD_TIMEOUT as expected: {}", e);
                    assert_eq!(e.get_error_code(), ERROR_CMD_TIMEOUT);
                })
                .or_else(|| {
                    panic!("CC_NOP should return XCP error code ERROR_CMD_TIMEOUT");
                });
        }
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Check error responses with CC_SYNC
    debug!("Check error response handling");
    let res = xcp_client.command(CC_SYNC).await; // Check unknown command
    match res {
        Ok(_) => panic!("Should return error"),
        Err(e) => {
            e.downcast_ref::<XcpClientError>()
                .map(|e| {
                    assert_eq!(e.get_error_code(), CRC_CMD_SYNCH);
                    debug!("XCP error code CRC_CMD_SYNCH from SYNC as expected: {}", e);
                })
                .or_else(|| {
                    panic!("Should return XCP error from SYNC command");
                });
        }
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Upload/Load  A2L file and check EPK

    if load_a2l {
        // Upload A2L file from XCP server
        if upload_a2l {
            xcp_client.a2l_loader().await.unwrap();
        }
        // Load the A2L file from file
        else {
            // Send XCP GET_ID GET_ID XCP_IDT_ASAM_NAME to obtain the A2L filename
            info!("XCP GET_ID XCP_IDT_ASAM_NAME");
            let res = xcp_client.get_id(XCP_IDT_ASAM_NAME).await;
            let a2l_name = match res {
                Ok((_, Some(id))) => id,
                Err(e) => {
                    panic!("GET_ID failed, Error: {}", e);
                }
                _ => {
                    panic!("Empty string");
                }
            };
            info!("A2l file name from GET_ID IDT_ASAM_NAME = {}", a2l_name);

            // Check A2l file exists on disk
            let a2l_filename = format!("{}.a2l", a2l_name);
            let info = std::fs::metadata(&a2l_filename).unwrap();
            trace!("A2l file info: {:#?}", info);
            assert!(info.len() > 0);

            // Load A2L file from file not implemented yet
            unimplemented!("Load A2L file from file not implemented yet");
        }

        // Check EPK
        // EPK addr is always 0x80000000 and len is hardcoded to 8
        let res = xcp_client.short_upload(0x80000000, 0, 8).await;
        let resp: Vec<u8> = match res {
            Err(e) => {
                panic!("Could not upload EPK, Error: {}", e);
            }
            Ok(r) => r,
        };
        let epk = resp[1..=8].to_vec();
        let epk_string = String::from_utf8(epk.clone()).unwrap();
        info!("Upload EPK = {} {:?}", epk_string, epk);
        debug!("A2l EPK = {}", xcp_client.a2l_epk().unwrap());
        //assert_eq!(epk_string.as_str(), xcp_client.a2l_epk().unwrap(), "EPK mismatch");
    }

    // Check the DAQ clock
    debug!("Start clock test");
    let t10 = Instant::now();
    let t1 = xcp_client.get_daq_clock().await.unwrap();
    tokio::time::sleep(Duration::from_micros(1000)).await;
    let t20 = t10.elapsed();
    let t2 = xcp_client.get_daq_clock().await.unwrap();
    let dt12 = (t2 - t1) / 1000;
    let dt120 = t20.as_micros() as u64;
    let diff = dt120 as i64 - dt12 as i64;
    if !(-100..=100).contains(&diff) {
        warn!("DAQ clock too inaccurate");
        warn!("t1 = {}ns, t2 = {}ns, dt={}us / elapsed={}us diff={}", t1, t2, dt12, dt120, diff);
    }
    //assert!(dt12 > dt120 - 400, "DAQ clock too slow");
    //assert!(dt12 < dt120 + 400, "DAQ clock too fast");

    (xcp_client, daq_decoder)
}

// Test shutdown
// Disconnect from XCP server
pub async fn test_disconnect(xcp_client: &mut XcpClient) {
    let mut error_state = false;

    // Disconnect from XCP server
    info!("Disconnect from XCP server");
    xcp_client
        .disconnect()
        .await
        .map_err(|e| {
            error_state = true;
            error!("Disconnect failed: {:?}", e);
        })
        .ok();
}

//-------------------------------------------------------------------------------------------------------------------------------------

pub async fn test_executor(test_mode_cal: TestModeCal, test_mode_daq: TestModeDaq, daq_test_duration_ms: u64, task_count: usize, signal_count: usize, task_cycle_us: u64) {
    let mut error_state = false;

    let load_a2l = test_mode_cal != TestModeCal::None || test_mode_daq != TestModeDaq::None;
    let (mut xcp_client, daq_decoder) = test_setup(task_count, load_a2l, true).await;

    // Cal or Daq test enabled
    if test_mode_cal != TestModeCal::None || test_mode_daq != TestModeDaq::None {
        if test_mode_daq != TestModeDaq::None {
            tokio::time::sleep(Duration::from_micros(10000)).await;

            if test_mode_daq != TestModeDaq::DaqPerformance {
                // Set counter_max to 15
                // Create a calibration object for counter_max
                let counter_max = xcp_client
                    .create_calibration_object("cal_seg.counter_max")
                    .await
                    .expect("could not create calibration object cal_seg.counter_max");
                let v = xcp_client.get_value_u64(counter_max);
                assert_eq!(v, 0xFFFF);
                xcp_client.set_value_u64(counter_max, 15).await.unwrap();
                tokio::time::sleep(Duration::from_micros(100000)).await;
            } // !test_mode_daq == TestModeDaq::DaqPerformance

            //-------------------------------------------------------------------------------------------------------------------------------------
            // DAQ test

            info!("Start DAQ test");
            let (test_ok, actual_duration_ms) = test_daq(&mut xcp_client, test_mode_daq, signal_count, daq_test_duration_ms, task_count, task_cycle_us).await;
            let packets_lost = DAQ_PACKETS_LOST.load(std::sync::atomic::Ordering::SeqCst);
            let counter_errors = DAQ_COUNTER_ERRORS.load(std::sync::atomic::Ordering::SeqCst);
            info!(
                "DAQ test done, duration = {}ms, packet_loss = {}, counter_error = {}",
                actual_duration_ms, packets_lost, counter_errors
            );

            if test_ok {
                let d = daq_decoder.lock();
                info!("Daq test results:");
                info!("  cycle time = {}us", task_cycle_us);
                info!("  task count = {}", d.task_count);
                info!("  signals = {}", d.task_count * (5 + 8));
                info!("  events per task = {}", d.daq_events[0]);
                info!("  events total = {}", d.tot_events);
                info!("  bytes total = {}", d.tot_bytes);
                info!("  events/s = {:.0}", d.tot_events as f64 / actual_duration_ms as f64 * 1000.0);
                info!("  datarate = {:.3} MByte/s", (d.tot_bytes as f64) / 1000.0 / actual_duration_ms as f64);
                if d.packets_lost > 0 {
                    warn!("  packets lost = {}", d.packets_lost);
                }
                if d.packets_lost > 0 {
                    warn!("  counter errors = {}", d.counter_errors);
                }
                let avg_cycletime_us = (actual_duration_ms as f64 * 1000.0) / d.daq_events[0] as f64;
                info!("  average task cycle time = {:.1}us", avg_cycletime_us,);

                // Test asserts
                // Performance test does not assert packet_loss and counter errors
                assert_eq!(d.daq_max, (task_count - 1) as u16);
                assert_ne!(d.tot_events, 0);
                assert!(d.daq_events[0] > 0);
                assert_eq!(d.odt_max, 0);
                if test_mode_daq != TestModeDaq::DaqPerformance {
                    assert_eq!(d.counter_errors, 0);
                    // @@@@ TODO reenable packet loss zero requirement
                    // assert_eq!(d.packets_lost, 0);
                }
            } else {
                error!("Daq test failed");
                error_state = true;
            }
        } // test_mode_daq == TestModeDaq::DaqSingleThread || test_mode_daq == TestModeDaq::MultiThreadDAQ

        //-------------------------------------------------------------------------------------------------------------------------------------
        // Calibration test
        if !error_state && (test_mode_cal == TestModeCal::Cal) {
            let error = test_calibration(&mut xcp_client, task_cycle_us).await;
            if error {
                info!("Consistent calibration test passed");
            } else {
                error!("Consistent calibration test failed");
                error_state = true;
            }

            // Consistency not implemented yet
            // let error = test_consistent_calibration(&mut xcp_client).await;
            // if error {
            //     info!("Consistent calibration test passed");
            // } else {
            //     error!("Consistent calibration test failed");
            //     error_state = true;
            // }
        } // !error_state && (test_mode_cal == TestModeCal::Cal)

        //-------------------------------------------------------------------------------------------------------------------------------------
        // Stop test tasks by calibration value cal_seg.run
        if test_mode_daq != TestModeDaq::DaqPerformance {
            debug!("Stop test tasks");
            let run = xcp_client.create_calibration_object("cal_seg.run").await.expect("could not create calibration object run");
            let v = xcp_client.get_value_u64(run);
            assert_eq!(v, 1);
            xcp_client
                .set_value_u64(run, 0)
                .await
                .map_err(|e| {
                    error_state = true;
                    error!("Calibrarion of calseg.run failed: {:?}", e);
                })
                .ok();
            tokio::time::sleep(Duration::from_millis(1000)).await; // Give the user task some time to finish
        }
    }

    test_disconnect(&mut xcp_client).await;

    if error_state {
        panic!("Test failed");
    } else {
        info!("Test passed");
    }
}
