//-----------------------------------------------------------------------------
// Module xcp_test_executor
// Runs various tests agains a XCP server on local host UDP port 5555

#![allow(unused_imports)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

use log::{debug, error, info, trace, warn};

use tokio::time::{Duration, Instant};

use xcp::Xcp;
use xcp_client::a2l::*;
use xcp_client::xcp_client::*;

pub use xcp_client::xcp_client::XCPTL_MAX_SEGMENT_SIZE;

//-----------------------------------------------------------------------------

// Logging
pub const OPTION_LOG_LEVEL: xcp::XcpLogLevel = xcp::XcpLogLevel::Info;
pub const OPTION_XCP_LOG_LEVEL: xcp::XcpLogLevel = xcp::XcpLogLevel::Info;

// Test parameters
pub const MULTI_THREAD_TASK_COUNT: usize = 16; // Number of threads
pub const DAQ_TEST_TASK_SLEEP_TIME_US: u64 = 100; // Measurement thread task cycle time in us
const DAQ_TEST_DURATION_MS: u64 = 6000; // DAQ test duration, 6s to get a nano second 32 bit overflow while checking timestamp monotony
const CAL_TEST_MAX_ITER: u32 = 4000; // Number of calibrations
const CAL_TEST_TASK_SLEEP_TIME_US: u64 = 50; // Checking task cycle time in us

//------------------------------------------------------------------------
// Handle incomming SERV_TEXT data

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
        print!("SERV_TEXT: ");
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
// Handle incomming DAQ data
// Create some test diagnostic data

#[derive(Debug, Clone, Copy)]
struct DaqDecoder {
    timestamp_resolution: u64,
    tot_events: u32,
    packets_lost: u32,
    counter_errors: u32,
    daq_max: u16,
    odt_max: u8,
    daq_timestamp: [u64; MULTI_THREAD_TASK_COUNT],
    daq_events: [u32; MULTI_THREAD_TASK_COUNT],
    max_counter: [u32; MULTI_THREAD_TASK_COUNT],
    last_counter: [u32; MULTI_THREAD_TASK_COUNT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            timestamp_resolution: 1,
            tot_events: 0,
            packets_lost: 0,
            counter_errors: 0,
            daq_max: 0,
            odt_max: 0,
            daq_timestamp: [0; MULTI_THREAD_TASK_COUNT],
            daq_events: [0; MULTI_THREAD_TASK_COUNT],
            max_counter: [0; MULTI_THREAD_TASK_COUNT],
            last_counter: [0; MULTI_THREAD_TASK_COUNT],
        }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    // Set start time and reset
    fn start(&mut self, timestamp: u64) {
        self.tot_events = 0;
        self.packets_lost = 0;
        self.counter_errors = 0;
        self.daq_max = 0;
        self.odt_max = 0;
        for i in 0..MULTI_THREAD_TASK_COUNT {
            self.daq_timestamp[i] = timestamp;
            self.daq_events[i] = 0;
            self.max_counter[i] = 0;
            self.last_counter[i] = 0;
        }
    }

    // Set timestamp resolution
    fn set_timestamp_resolution(&mut self, timestamp_resolution: u64) {
        self.timestamp_resolution = timestamp_resolution;
    }

    // Handle incomming DAQ DTOs from XCP server
    fn decode(&mut self, lost: u32, daq: u16, odt: u8, timestamp: u32, data: &[u8]) {
        assert!(daq < MULTI_THREAD_TASK_COUNT as u16);

        if lost > 0 {
            self.packets_lost += lost;
            warn!("packet loss = {}, total = {}", lost, self.packets_lost);
        }

        if daq > self.daq_max {
            self.daq_max = daq;
        }

        // Decode raw timestamp as u64
        // Check declining timestamps
        if odt == 0 {
            let t_last = self.daq_timestamp[daq as usize];
            let tl = (t_last & 0xFFFFFFFF) as u32;
            let mut th = (t_last >> 32) as u32;
            if timestamp < tl {
                th += 1;
            }
            let t = timestamp as u64 | (th as u64) << 32;
            if t < t_last {
                warn!("Timestamp of daq {} declining {} -> {}", daq, t_last, t);
            }
            self.daq_timestamp[daq as usize] = t;
        }

        // Hardcoded decoding of data (only one ODT)
        assert!(odt == 0);
        if odt == 0 && data.len() >= 8 {
            let o = 0;

            // Check counter_max (+0) and counter (+4)
            let counter_max = data[o] as u32 | (data[o + 1] as u32) << 8 | (data[o + 2] as u32) << 16 | (data[o + 3] as u32) << 24;
            let counter = data[o + 4] as u32 | (data[o + 5] as u32) << 8 | (data[o + 6] as u32) << 16 | (data[o + 7] as u32) << 24;
            if counter_max > 255 || counter > 255 || counter > counter_max {
                warn!("counter_max={}, counter={}", counter_max, counter);
            }
            //assert!(counter <= 255, "counter={}", counter);
            //assert!(counter <= counter_max, "counter={} counter_max={}", counter, counter_max);
            if counter_max >= self.max_counter[daq as usize] {
                self.max_counter[daq as usize] = counter_max;
            }

            // Check cal_test pattern (+8)
            if data.len() >= 16 {
                let cal_test = data[o + 8] as u64
                    | (data[o + 9] as u64) << 8
                    | (data[o + 10] as u64) << 16
                    | (data[o + 11] as u64) << 24
                    | (data[o + 12] as u64) << 32
                    | (data[o + 13] as u64) << 40
                    | (data[o + 14] as u64) << 48
                    | (data[o + 15] as u64) << 56;
                assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
            }

            // Check each counter is incrementing
            if self.daq_events[daq as usize] != 0 && counter != self.last_counter[daq as usize] + 1 && counter != 0 && daq != 0 {
                trace!("counter error: daq={} {} -> {} max={} ", daq, self.last_counter[daq as usize], counter, counter_max,);
            }
            self.last_counter[daq as usize] = counter;

            trace!(
                "DAQ: daq = {}, odt = {} timestamp = {} counter={}, counter_max={} (rest={:?})",
                daq,
                odt,
                timestamp,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestModeDaq {
    None,
    SingleThreadDAQ,
    MultiThreadDAQ,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestModeCal {
    None,
    Cal,
}

pub async fn xcp_test_executor(xcp: &Xcp, test_mode_cal: TestModeCal, test_mode_daq: TestModeDaq, a2l_file: &str, a2l_upload: bool) {
    let mut error_state = false;

    tokio::time::sleep(Duration::from_millis(500)).await;
    info!("Start test executor in {:?}", test_mode_daq);

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Create xcp_client and connect the XCP server
    info!("XCP CONNECT");
    let dest_addr = "127.0.0.1:5555".parse().unwrap();
    let local_addr = "0.0.0.0:0".parse().unwrap();
    info!("  dest_addr: {}", dest_addr);
    info!("  local_addr: {}", local_addr);
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    let serv_text_decoder = ServTextDecoder::new();
    xcp_client.connect(Arc::clone(&daq_decoder), serv_text_decoder).await.unwrap();
    tokio::time::sleep(Duration::from_micros(10000)).await;
    assert!(xcp.get_session_status().contains(xcp::XcpSessionStatus::SS_CONNECTED));

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Check command timeout using a command CC_NOP (non standard) without response
    info!("Check command timeout handling");
    let res = xcp_client.command(CC_NOP).await; // Check unknown command
    match res {
        Ok(_) => panic!("Should timeout"),
        Err(e) => {
            e.downcast_ref::<XcpError>()
                .map(|e| {
                    debug!("XCP error code ERROR_CMD_TIMEOUT as expected: {}", e);
                    assert_eq!(e.get_error_code(), ERROR_CMD_TIMEOUT);
                })
                .or_else(|| {
                    info!("XCP session status: {:?}", xcp.get_session_status());
                    panic!("CC_NOP should return XCP error code ERROR_CMD_TIMEOUT");
                });
        }
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Check error responses with CC_SYNC
    info!("Check error response handling");
    let res = xcp_client.command(CC_SYNC).await; // Check unknown command
    match res {
        Ok(_) => panic!("Should return error"),
        Err(e) => {
            e.downcast_ref::<XcpError>()
                .map(|e| {
                    info!("XCP session status: {:?}", xcp.get_session_status());
                    assert_eq!(e.get_error_code(), CRC_CMD_SYNCH);
                    debug!("XCP error code CRC_CMD_SYNCH from SYNC as expected: {}", e);
                })
                .or_else(|| {
                    info!("XCP session status: {:?}", xcp.get_session_status());
                    panic!("Should return XCP error from SYNC command");
                });
        }
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Get id
    info!("XCP GET_ID XCP_IDT_ASAM_NAME");
    let res = xcp_client.get_id(XCP_IDT_ASAM_NAME).await;
    let asam_name = match res {
        Ok((_, Some(id))) => id,
        Err(e) => {
            panic!("GET_ID failed, Error: {}", e);
        }
        _ => {
            panic!("Empty string");
        }
    };
    let a2l_file_name = format!("{}.a2l", asam_name);
    info!("A2l file name = {}", a2l_file_name);
    assert_eq!(a2l_file, a2l_file_name.as_str());
    // Check A2l file exists
    let info = std::fs::metadata(&a2l_file_name).unwrap();
    trace!("A2l file info: {:#?}", info);
    assert!(info.len() > 0);

    if test_mode_cal != TestModeCal::None || test_mode_daq != TestModeDaq::None {
        //-------------------------------------------------------------------------------------------------------------------------------------
        // Upload or just load A2L file
        info!("Read A2l {}, upload={}", a2l_file_name, a2l_upload);
        xcp_client.load_a2l(&a2l_file_name, a2l_upload, false).await.unwrap();
        tokio::time::sleep(Duration::from_micros(10000)).await;

        //-------------------------------------------------------------------------------------------------------------------------------------
        // Check EPK upload
        let res = xcp_client.short_upload(0x80000000, 0, 8).await;
        let resp: Vec<u8> = match res {
            Err(e) => {
                panic!("Could not upload EPK, Error: {}", e);
            }
            Ok(r) => r,
        };
        let epk = resp[1..=8].to_vec();
        let epk_string = String::from_utf8(epk.clone()).unwrap();
        info!("Upload EPK = {} {:?}\n", epk_string, epk);
        assert_eq!(epk_string, "EPK_TEST", "Unexpected EPK string");

        //-------------------------------------------------------------------------------------------------------------------------------------
        // Create calibration objects for CalPage1.cycle_time_us and CalPage1.run

        // Create calibration object for CalPage1.cycle_time_us
        debug!("Create calibration object CalPage1.cycle_time_us");
        let cycle_time_us = xcp_client
            .create_calibration_object("CalPage1.cycle_time_us")
            .await
            .expect("could not create calibration object CalPage1.cycle_time_us");

        // Create a calibration object for CalPage.run
        debug!("Create calibration object CalPage1.run");
        let run = xcp_client.create_calibration_object("CalPage1.run").await.expect("could not create calibration object CalPage1.run");
        let v = xcp_client.get_value_u64(run);
        assert_eq!(v, 1);

        //-------------------------------------------------------------------------------------------------------------------------------------
        // DAQ test single_thread or multi_thread
        if test_mode_daq == TestModeDaq::SingleThreadDAQ || test_mode_daq == TestModeDaq::MultiThreadDAQ {
            tokio::time::sleep(Duration::from_micros(10000)).await;

            // Create a calibration object for CalPage1.counter_max
            // Set counter_max to 15
            let counter_max = xcp_client
                .create_calibration_object("CalPage1.counter_max")
                .await
                .expect("could not create calibration object CalPage1.counter_max");
            xcp_client.set_value_u64(counter_max, 15).await.unwrap();
            tokio::time::sleep(Duration::from_micros(100000)).await;

            // Set cycle time
            xcp_client.set_value_u64(cycle_time_us, DAQ_TEST_TASK_SLEEP_TIME_US).await.unwrap();

            // Check the DAQ clock
            info!("Start clock test");
            let t10 = Instant::now();
            let t1 = xcp_client.get_daq_clock().await.unwrap();
            tokio::time::sleep(Duration::from_micros(1000)).await;
            let t20 = t10.elapsed();
            let t2 = xcp_client.get_daq_clock().await.unwrap();
            let dt12 = (t2 - t1) / 1000;
            let dt120 = t20.as_micros() as u64;
            info!("t1 = {}ns, t2 = {}ns, dt={}us / elapsed={}us", t1, t2, dt12, dt120);
            assert!(dt12 > dt120 - 100, "DAQ clock too slow");
            assert!(dt12 < dt120 + 100, "DAQ clock too fast");

            info!("Start data acquisition test");

            // Measurement test loop
            // Create a measurement DAQ list with all instances MULTI_THREAD_TASK_COUNT of measurement counter and counter_max
            // Hard coded order and size in DaqDecoder (counter_max, counter, cal_test, ...)
            let bytes_per_event: u32 = if test_mode_daq == TestModeDaq::MultiThreadDAQ {
                let mut n = 0;
                for i in 1..=MULTI_THREAD_TASK_COUNT {
                    let counter = "counter_".to_string() + &i.to_string();
                    let counter_max = "counter_max_".to_string() + &i.to_string();
                    let cal_test = "cal_test_".to_string() + &i.to_string();
                    let loop_counter = "loop_counter_".to_string() + &i.to_string();
                    let changes = "changes_".to_string() + &i.to_string();
                    xcp_client.create_measurement_object(counter_max.as_str()).unwrap(); // +0
                    xcp_client.create_measurement_object(counter.as_str()).unwrap(); // +4
                    xcp_client.create_measurement_object(cal_test.as_str()).unwrap(); // +8
                    xcp_client.create_measurement_object(loop_counter.as_str()).unwrap(); // +16
                    xcp_client.create_measurement_object(changes.as_str()).unwrap(); //
                    for j in 0.. {
                        let name = format!("test{}_{}", j, i);
                        let res = xcp_client.create_measurement_object(name.as_str());
                        if res.is_none() {
                            n = j;
                            break;
                        }
                    }
                }
                32 + n * 8
            } else {
                xcp_client.create_measurement_object("counter_max").unwrap();
                xcp_client.create_measurement_object("counter").unwrap();
                xcp_client.create_measurement_object("cal_test").unwrap();
                16
            };
            xcp_client.start_measurement().await.unwrap();

            // Test for DURATION_DAQ_TEST_MS time, do a calibration of counter_max to 255 in the middle of the time
            let starttime = Instant::now();
            tokio::time::sleep(Duration::from_millis(DAQ_TEST_DURATION_MS / 2)).await;
            xcp_client.set_value_u64(counter_max, 255).await.unwrap(); // Calibrate counter_max
            tokio::time::sleep(Duration::from_millis(DAQ_TEST_DURATION_MS / 2)).await;
            let duration_s = starttime.elapsed().as_secs_f64();
            let duration_ms = duration_s * 1000.0;

            // Stop DAQ
            let res = xcp_client.stop_measurement().await;
            match res {
                Ok(_) => {
                    info!("DAQ stopped");
                }
                Err(e) => {
                    error!("DAQ stop failed: {:?}", e);
                    error_state = true;
                }
            }

            // Wait some time to be sure the queue is emptied
            // The XCP server will not respond to STOP while the queue is not empty
            // But the queue of the client may still contain data or the control channel may need some more time
            tokio::time::sleep(Duration::from_millis(250)).await;

            // Check results
            {
                let d = daq_decoder.lock().unwrap();
                info!("DAQ test cycle time = {}us", DAQ_TEST_TASK_SLEEP_TIME_US);
                if test_mode_daq == TestModeDaq::MultiThreadDAQ {
                    info!("DAQ test thread count = {}", MULTI_THREAD_TASK_COUNT);
                    info!(
                        "DAQ test target data rate {} MByte/s",
                        (1.0 / DAQ_TEST_TASK_SLEEP_TIME_US as f64) * (bytes_per_event * MULTI_THREAD_TASK_COUNT as u32) as f64
                    );
                }
                info!("  signals = {}", MULTI_THREAD_TASK_COUNT * 8);
                info!("  cycles = {}", d.daq_events[0]);
                info!("  packets lost = {}", d.packets_lost);
                info!("  counter errors = {}", d.counter_errors);
                info!("  events = {}", d.tot_events);
                info!("  events per sec= {:.0}", d.tot_events as f64 / duration_s);
                info!("  bytes per event = {}", bytes_per_event);
                info!("  bytes total = {}", bytes_per_event as u64 * d.tot_events as u64);
                assert_ne!(d.tot_events, 0);
                assert!(d.daq_events[0] > 0);
                info!("  test duration = {:.3}ms", duration_ms);
                info!("  average datarate = {:.3} MByte/s", (bytes_per_event as f64 * d.tot_events as f64) / 1000.0 / duration_ms,);
                assert!(duration_ms > DAQ_TEST_DURATION_MS as f64 && duration_ms < DAQ_TEST_DURATION_MS as f64 + 100.0);
                let avg_cycletime_us = (duration_ms * 1000.0) / d.daq_events[0] as f64;
                info!("  average task cycle time = {}us", avg_cycletime_us,);
                if test_mode_daq == TestModeDaq::MultiThreadDAQ {
                    assert_eq!(d.daq_max, (MULTI_THREAD_TASK_COUNT - 1) as u16);
                    // Check all max counters are now 255
                    for i in 0..MULTI_THREAD_TASK_COUNT {
                        assert_eq!(d.max_counter[i], 255);
                    }
                } else {
                    assert_eq!(d.daq_max, 0);
                    assert_eq!(d.max_counter[0], 255); // @@@@
                }
                assert_eq!(d.odt_max, 0);
                assert_eq!(d.counter_errors, 0);
                assert_eq!(d.packets_lost, 0);
            }
        }

        //-------------------------------------------------------------------------------------------------------------------------------------
        //-------------------------------------------------------------------------------------------------------------------------------------
        // Calibration test
        if !error_state && (test_mode_cal == TestModeCal::Cal) {
            info!("Start calibration test");

            // Wait some time to be sure the queue is emptied
            // The XCP server should not respond to STOP while the queue is not empty
            // But the queue of the client may still contain data or the control channel may need some time
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Test signed
            debug!("Create calibration object CalPage1.test_i16");
            let test_i32 = xcp_client
                .create_calibration_object("CalPage1.TestInts.test_i16")
                .await
                .expect("could not create calibration object CalPage1.test_i16");
            let v = xcp_client.get_value_i64(test_i32);
            debug!("test_i32 = {}", v);
            xcp_client.set_value_i64(test_i32, 1).await.unwrap();
            let v = xcp_client.get_value_i64(test_i32);
            debug!("test_i32 = {}", v);
            xcp_client.set_value_i64(test_i32, -1).await.unwrap();
            let v = xcp_client.get_value_i64(test_i32);
            debug!("test_i32 = {}", v);

            // Check page switching
            // Check page is ram
            info!("Check ecu cal page");
            let mut page: u8 = xcp_client.get_ecu_page().await.unwrap();
            assert!(page == 0);
            page = xcp_client.get_xcp_page().await.unwrap();
            assert!(page == 0);

            // Mark the ram page in variable cal_seg.page
            let mut cal_seg_page = xcp_client.create_calibration_object("CalPage1.page").await.expect("could not create calibration object CalPage1.page");
            xcp_client // init page variable in ram page of cal_seg
                .set_value_u64(cal_seg_page, 0)
                .await
                .unwrap();
            // Switch to default
            xcp_client.set_ecu_page(1).await.unwrap();
            xcp_client.set_xcp_page(1).await.unwrap();
            tokio::time::sleep(Duration::from_micros(100000)).await;
            // Check if cal_seg.page marker is default
            cal_seg_page = xcp_client.create_calibration_object("CalPage1.page").await.expect("could not create calibration object CalPage1.page");
            page = xcp_client.get_value_u64(cal_seg_page) as u8;
            assert_eq!(page, 1);
            // Check if get cal page returns default
            page = xcp_client.get_xcp_page().await.unwrap();
            assert_eq!(page, 1);
            page = xcp_client.get_ecu_page().await.unwrap();
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
                xcp_client.set_value_u64(cycle_time_us, CAL_TEST_TASK_SLEEP_TIME_US).await.unwrap();

                // Create calibration variable CalPage1.cal_test
                let res = a2l_reader::a2l_find_characteristic(xcp_client.get_a2l_file().unwrap(), "CalPage1.cal_test").unwrap();
                let addr_cal_test = res.0.addr;
                debug!("download cal_test = 0x{:X}\n", res.0.addr);

                // Calibration loop
                // Set calibration variable cal_test to a defined pattern which will be checked by the server application task
                let start_time = Instant::now();
                for i in 0..CAL_TEST_MAX_ITER {
                    let cal_test = i as u64 + (((i as u64) << 32) ^ 0x5555555500000000u64); // The server task will check this pattern and panic if it is wrong
                    trace!("download cal_test = {:X}", cal_test);
                    xcp_client // SHORT_DOWNLOAD cal_seg.test_u64
                        .short_download(addr_cal_test, 0, &cal_test.to_le_bytes())
                        .await
                        .map_err(|e| {
                            error_state = true;
                            error!("download CalPage1.cal_test failed: {:?}", e);
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
            }
        }

        // Stop test task
        info!("Stop test tasks");
        xcp_client
            .set_value_u64(run, 0)
            .await
            .map_err(|e| {
                error_state = true;
                error!("Calibrarion of calseg.run failed: {:?}", e);
            })
            .ok();

        tokio::time::sleep(Duration::from_micros(100000)).await;
    }

    // Disconnect
    info!("Disconnect from XCP server");
    xcp_client
        .disconnect()
        .await
        .map_err(|e| {
            error_state = true;
            error!("Disconnect failed: {:?}", e);
        })
        .ok();

    if error_state {
        panic!("Test failed");
    }

    info!("Test passed");
}
