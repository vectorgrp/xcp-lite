//-----------------------------------------------------------------------------
// Module test_executor
// Runs various tests agains a XCP server on local host UDP port 5555

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use tokio::time::{Duration, Instant};

use xcp::Xcp;

use xcp::XcpLogLevel;
use xcp_client::a2l::*;
use xcp_client::xcp_client::*;

// DAQ test parameters
pub const MULTI_THREAD_TASK_COUNT: usize = 50; // No of signals = MULTI_THREAD_TASK_COUNT*8
const DURATION_DAQ_TEST_MS: u64 = 2000; // ms
const CYCLE_TIME_DAQ_TEST_US: u64 = 250; // us

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
    tot_events: u32,
    daq0_timestamp: u32,
    daq0_timestamp_max: u32,
    daq0_timestamp_min: u32,
    daq_max: u8,
    odt_max: u8,
    daq_events: [u32; MULTI_THREAD_TASK_COUNT],
    max_counter: [u32; MULTI_THREAD_TASK_COUNT],
    last_counter: [u32; MULTI_THREAD_TASK_COUNT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            tot_events: 0,
            daq_events: [0; MULTI_THREAD_TASK_COUNT],
            daq0_timestamp: 0,
            daq0_timestamp_max: 0,
            daq0_timestamp_min: 0xFFFFFFFF,
            daq_max: 0,
            odt_max: 0,
            max_counter: [0; MULTI_THREAD_TASK_COUNT],
            last_counter: [0; MULTI_THREAD_TASK_COUNT],
        }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    // Handle incomming DAQ data from XCP server
    fn decode(&mut self, _control: &XcpTaskControl, data: &[u8]) {
        // Decode DAQ data
        let mut daq = data[1];
        if (daq & 0x80) != 0 {
            error!("DAQ queue overflow!");
            daq &= 0x7F;
        }
        if daq > self.daq_max {
            self.daq_max = daq;
        }
        assert!(daq < MULTI_THREAD_TASK_COUNT as u8);
        let odt = data[0];
        if odt > self.odt_max {
            self.odt_max = odt;
        }
        assert!(odt != 0x80, "DAQ buffer overflow");
        assert!(odt == 0);
        if odt == 0 && data.len() >= 14 {
            let timestamp = data[2] as u32
                | (data[3] as u32) << 8
                | (data[4] as u32) << 16
                | (data[5] as u32) << 24;
            let counter_max = data[6] as u32
                | (data[7] as u32) << 8
                | (data[8] as u32) << 16
                | (data[9] as u32) << 24;
            let counter = data[10] as u32
                | (data[11] as u32) << 8
                | (data[12] as u32) << 16
                | (data[13] as u32) << 24;
            if data.len() >= 22 {
                let cal_test = data[14] as u64
                    | (data[15] as u64) << 8
                    | (data[16] as u64) << 16
                    | (data[17] as u64) << 24
                    | (data[18] as u64) << 32
                    | (data[19] as u64) << 40
                    | (data[20] as u64) << 48
                    | (data[21] as u64) << 56;
                // Check cal_test pattern
                assert_eq!((cal_test >> 32) ^ 0x55555555, cal_test & 0xFFFFFFFF);
            }
            //self.counter[daq as usize] = counter;
            if counter_max >= self.max_counter[daq as usize] {
                self.max_counter[daq as usize] = counter_max;
            }

            debug!(
                "DAQ: daq = {}, odt = {} timestamp = {} counter={}, counter_max={} (rest={:?})",
                daq,
                odt,
                timestamp,
                counter,
                counter_max,
                &data[6..]
            );

            // Check counter <= counter_max
            assert!(counter <= counter_max);

            // Check each counter is incrementing
            if self.daq_events[daq as usize] != 0
                && counter != self.last_counter[daq as usize] + 1
                && counter != 0
            {
                error!(
                    "counter error: counter={} counter_max={} last_counter={} ",
                    counter, counter_max, self.last_counter[daq as usize]
                );
            }
            self.last_counter[daq as usize] = counter;

            // Check timestamp of daq 0
            if daq == 0 {
                if self.daq_events[0] != 0 {
                    if timestamp < self.daq0_timestamp {
                        error!(
                            "declining timestamp: timestamp={} last={}",
                            timestamp, self.daq0_timestamp
                        );
                    } else {
                        let dt = timestamp - self.daq0_timestamp;
                        self.daq0_timestamp = timestamp;
                        if dt > self.daq0_timestamp_max {
                            self.daq0_timestamp_max = dt;
                        }
                        if dt < self.daq0_timestamp_min {
                            self.daq0_timestamp_min = dt;
                        }
                    }
                }
                self.daq0_timestamp = timestamp;
            }
            self.daq_events[daq as usize] += 1;
            self.tot_events += 1;
        }
    }
}

//-----------------------------------------------------------------------
// Execute tests
pub async fn test_executor(single_thread: bool, multi_thread: bool, log_level: XcpLogLevel) {
    tokio::time::sleep(Duration::from_millis(500)).await;
    info!("Start test executor");

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Create xcp_client and connect the XCP server
    info!("Connect");
    let dest_addr: Result<SocketAddr, _> = "127.0.0.1:5555".parse();
    let local_addr: Result<SocketAddr, _> = "0.0.0.0:0".parse();
    debug!("dest_addr: {:?}", dest_addr);
    debug!("local_addr: {:?}", local_addr);
    let mut xcp_client = XcpClient::new(dest_addr.unwrap(), local_addr.unwrap());

    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    let serv_text_decoder = ServTextDecoder::new();
    xcp_client
        .connect(Arc::clone(&daq_decoder), serv_text_decoder)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_micros(10000)).await;

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Check command timeout using a command CC_NOP (non standard) without response
    info!("Check timeout");
    let res = xcp_client.command(CC_NOP).await; // Check unknown command
    match res {
        Ok(_) => panic!("Should timeout"),
        Err(e) => {
            e.downcast_ref::<XcpError>()
                .map(|e| {
                    assert_eq!(e.get_error_code(), ERROR_CMD_TIMEOUT);
                    debug!("XCP error code ERROR_CMD_TIMEOUT as expected: {:?}", e);
                })
                .or_else(|| panic!("CC_NOP should return XCP error code ERROR_CMD_TIMEOUT"));
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
                    assert_eq!(e.get_error_code(), CRC_CMD_SYNCH);
                    debug!(
                        "XCP error code CRC_CMD_SYNCH from SYNC as expected: {:?}",
                        e
                    );
                })
                .or_else(|| panic!("Should return XCP error from SYNC command"));
        }
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Upload A2L file
    tokio::time::sleep(Duration::from_micros(10000)).await;
    info!("Upload A2l");
    Xcp::set_server_log_level(log_level); // upload will be too verbose
    xcp_client.upload_a2l().await.unwrap();
    tokio::time::sleep(Duration::from_micros(10000)).await;
    Xcp::set_server_log_level(log_level); // restore log level

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
    let run = xcp_client
        .create_calibration_object("CalPage1.run")
        .await
        .expect("could not create calibration object CalPage1.run");
    let v = xcp_client.get_value_u64(run);
    assert_eq!(v, 1);

    //-------------------------------------------------------------------------------------------------------------------------------------
    // DAQ test single_thread or multi_thread
    if single_thread || multi_thread {
        let log_level_daq_test = log_level;
        tokio::time::sleep(Duration::from_micros(10000)).await;
        info!("Start data acquisition test");
        Xcp::set_server_log_level(log_level_daq_test);

        // Create a calibration object for CalPage1.counter_max
        // Set counter_max to 15
        let counter_max = xcp_client
            .create_calibration_object("CalPage1.counter_max")
            .await
            .expect("could not create calibration object CalPage1.counter_max");
        xcp_client.set_value_u64(counter_max, 15).await.unwrap();

        // Set cycle time
        xcp_client
            .set_value_u64(cycle_time_us, CYCLE_TIME_DAQ_TEST_US)
            .await
            .unwrap(); // 1us

        // Measurement test loop
        // Create a measurement DAQ list with all instances MULTI_THREAD_TASK_COUNT of measurement counter and counter_max
        // Hard coded order and size in DaqDecoder (counter_max, counter, cal_test, ...)
        let mut bytes: u32 = 0;
        // for multi_thread
        if multi_thread {
            for i in 1..=MULTI_THREAD_TASK_COUNT {
                let counter = "counter_".to_string() + &i.to_string();
                let counter_max = "counter_max_".to_string() + &i.to_string();
                let cal_test = "cal_test_".to_string() + &i.to_string();
                let loop_counter = "loop_counter_".to_string() + &i.to_string();
                let changes = "changes_".to_string() + &i.to_string();
                let test1 = "test1_".to_string() + &i.to_string();
                let test2 = "test2_".to_string() + &i.to_string();
                let test3 = "test3_".to_string() + &i.to_string();
                let test4 = "test4_".to_string() + &i.to_string();

                xcp_client
                    .create_measurement_object(counter_max.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(counter.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(cal_test.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(loop_counter.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(changes.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(test1.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(test2.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(test3.as_str())
                    .unwrap();
                xcp_client
                    .create_measurement_object(test4.as_str())
                    .unwrap();

                bytes += 32 + 32; // counter 4 + counter_max 4 + cal_test 8 + loop_counter 8 + changes 8 + test1-4 32
            }
        }
        // for single_thread
        else {
            xcp_client.create_measurement_object("counter_max").unwrap();
            xcp_client.create_measurement_object("counter").unwrap();
            bytes += 8;
        }
        xcp_client.start_measurement().await.unwrap();

        // Test for DURATION_DAQ_TEST_MS time, do a calibration in the middle
        let starttime = Instant::now();
        tokio::time::sleep(Duration::from_millis(DURATION_DAQ_TEST_MS / 2)).await;
        xcp_client.set_value_u64(counter_max, 255).await.unwrap(); // Calibrate counter_max
        tokio::time::sleep(Duration::from_millis(DURATION_DAQ_TEST_MS / 2)).await;
        let dt = starttime.elapsed().as_secs_f64();
        let duration_ms = dt * 1000.0;

        // Stop DAQ
        xcp_client.stop_measurement().await.unwrap();

        // Check results
        {
            let d = daq_decoder.lock().unwrap();
            assert_ne!(d.tot_events, 0);
            info!("DAQ test cycle time = {}us", CYCLE_TIME_DAQ_TEST_US);
            if multi_thread {
                info!("DAQ test thread count = {}", MULTI_THREAD_TASK_COUNT);
                info!(
                    "DAQ test target data rate {} MByte/s",
                    (1.0 / CYCLE_TIME_DAQ_TEST_US as f64)
                        * (bytes * MULTI_THREAD_TASK_COUNT as u32) as f64
                );
            }
            info!("  signals = {}", MULTI_THREAD_TASK_COUNT * 8);
            info!("  cycles = {}", d.daq_events[0]);
            info!("  events = {}", d.tot_events);
            info!("  bytes per cycle = {}", bytes);
            assert!(d.tot_events > 0);
            assert!(d.daq_events[0] > 0);
            info!("  test duration = {:.3}ms", duration_ms);
            info!(
                "  average datarate = {:.3} MByte/s",
                (bytes as f64 * d.tot_events as f64) / 1000.0 / duration_ms,
            );
            assert!(
                duration_ms > DURATION_DAQ_TEST_MS as f64
                    && duration_ms < DURATION_DAQ_TEST_MS as f64 + 100.0
            );
            let avg_cycletime_us = (duration_ms * 1000.0) / d.daq_events[0] as f64;
            info!("  task cycle time:",);
            info!("    average = {}us", avg_cycletime_us,);
            info!("    min = {}us", d.daq0_timestamp_min);
            info!("    max = {}us", d.daq0_timestamp_max);
            let jitter = d.daq0_timestamp_max - d.daq0_timestamp_min;
            info!("    jitter = {}us", jitter);
            //assert!(jitter < 150); // us tolerance
            let diff: f64 = (d.daq0_timestamp_min as f64 - CYCLE_TIME_DAQ_TEST_US as f64).abs();
            info!("    ecu task cpu time = {:.1}us", diff);
            //assert!(diff < 50.0); // us tolerance
            if multi_thread {
                assert_eq!(d.daq_max, (MULTI_THREAD_TASK_COUNT - 1) as u8);
                // Check all max counters are now 255
                for i in 0..MULTI_THREAD_TASK_COUNT {
                    assert_eq!(d.max_counter[i], 255);
                }
            } else {
                assert_eq!(d.daq_max, 0);
                assert_eq!(d.max_counter[0], 255);
            }
            assert_eq!(d.odt_max, 0);
            // C
        }
    }
    //-------------------------------------------------------------------------------------------------------------------------------------
    // DAQ test multi_task
    else {
        xcp_client.create_measurement_object("counter_max").unwrap();
        xcp_client.create_measurement_object("counter").unwrap();
        xcp_client.start_measurement().await.unwrap();
        tokio::time::sleep(Duration::from_millis(1000)).await;
        xcp_client.stop_measurement().await.unwrap();
    }

    //-------------------------------------------------------------------------------------------------------------------------------------
    // Calibration test
    if single_thread || multi_thread {
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
        let mut cal_seg_page = xcp_client
            .create_calibration_object("CalPage1.page")
            .await
            .expect("could not create calibration object CalPage1.page");
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
            .create_calibration_object("CalPage1.page")
            .await
            .expect("could not create calibration object CalPage1.page");
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
            const MAX_ITER: u32 = 10000;
            const TASK_SLEEP_TIME_US: u64 = 50; // us thread
            const LOG_LEVEL: XcpLogLevel = XcpLogLevel::Warn; // for XCP server

            tokio::time::sleep(Duration::from_micros(10000)).await;
            info!("start calibration test");

            // Set task cycle time to TASK_SLEEP_TIME_US
            xcp_client
                .set_value_u64(cycle_time_us, TASK_SLEEP_TIME_US)
                .await
                .unwrap();
            Xcp::set_server_log_level(LOG_LEVEL);

            // Create calibration variable CalPage1.cal_test
            let res = a2l_reader::a2l_find_characteristic(
                xcp_client.get_a2l_file().unwrap(),
                "CalPage1.cal_test",
            )
            .unwrap();
            let addr_cal_test = res.0.addr;
            debug!("download cal_test = 0x{:X}\n", res.0.addr);

            // Calibration loop
            // Set calibration variable cal_test to a defined pattern which will be checked by the server application task
            let start_time = Instant::now();
            for i in 0..MAX_ITER {
                let cal_test = i as u64 + (((i as u64) << 32) ^ 0x5555555500000000u64); // The server task will check this pattern and panic if it is wrong
                trace!("download cal_test = {:X}", cal_test);

                xcp_client // SHORT_DOWNLOAD cal_seg.test_u64
                    .short_download(addr_cal_test, 0, &cal_test.to_le_bytes())
                    .await
                    .unwrap();
            }
            let elapsed_time = start_time.elapsed().as_micros();
            info!(
                "calibration test loop done, {} iterations, duration={}ms, {}us per download, {:.1} KBytes/s",
                MAX_ITER,
                elapsed_time / 1000,
                elapsed_time as f64 / MAX_ITER as f64,
                MAX_ITER as f64 * 8000.0 / elapsed_time as f64
            );
        }
    }

    // Stop test task
    xcp_client.set_value_u64(run, 0).await.unwrap();

    // Disconnect
    info!("DISCONNECT");
    xcp_client.disconnect().await.unwrap();

    //std::fs::remove_file("test_upload.a2l").ok();
}
