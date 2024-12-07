// cargo bench
//

#![allow(unused_assignments)]
#![allow(unused_imports)]

use log::{debug, error, info, trace, warn};

use parking_lot::Mutex;
use std::{collections::HashMap, fmt::Debug, sync::Arc, thread, time::Duration};

use xcp::*;
use xcp_client::xcp_client::*;
use xcp_type_description::prelude::*;

use criterion::{criterion_group, criterion_main, Criterion};
//-----------------------------------------------------------------------------
// Calibration parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage {
    #[type_description(comment = "Amplitude value")]
    #[type_description(min = "0")]
    #[type_description(max = "10000.0")]
    ampl: f64,

    delay: u32,
}

const CAL_PAGE: CalPage = CalPage { ampl: 123.456, delay: 100 };

//-----------------------------------------------------------------------------
// XCP client

// Handle incomming DAQ data

#[derive(Debug, Clone, Copy)]
struct DaqDecoder {
    event_count: u64,
    event_lost_count: u64,
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            event_count: 0,
            event_lost_count: 0,
        }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    fn start(&mut self, _odt_entries: Vec<Vec<OdtEntry>>, _timestamp: u64) {
        self.event_count = 0;
        self.event_lost_count = 0;
    }

    fn set_daq_properties(&mut self, _timestamp_resolution: u64, _daq_header_size: u8) {}

    fn decode(&mut self, lost: u32, _data: &[u8]) {
        self.event_count += 1;
        self.event_lost_count += lost as u64;
    }
}

// Handle incomming SERV_TEXT data

#[derive(Debug, Clone, Copy)]
struct ServTextDecoder;

impl ServTextDecoder {
    pub fn new() -> ServTextDecoder {
        ServTextDecoder {}
    }
}

impl XcpTextDecoder for ServTextDecoder {
    fn decode(&self, _data: &[u8]) {}
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ClientMode {
    Wait,
    Calibrate,
    Measure,
    Stop,
}

// XCP client
async fn xcp_client(dest_addr: std::net::SocketAddr, local_addr: std::net::SocketAddr, mode: Arc<parking_lot::Mutex<ClientMode>>) -> Result<(), Box<dyn std::error::Error>> {
    println!("XCP client ");

    // Create xcp_client
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;

    // Upload A2L file
    info!("Upload A2L file");
    xcp_client.upload_a2l(true).await?;

    // Create a calibration object for CalPage.ampl
    info!("Create calibration object CalPage.ampl");
    let ampl = xcp_client
        .create_calibration_object("CalPage.ampl")
        .await
        .expect("Failed to create calibration object for CalPage.ampl");
    let v = xcp_client.get_value_f64(ampl);
    info!("CalPage.ampl = {}", v);

    let mut last_mode = *mode.lock();

    loop {
        let current_mode = *mode.lock();
        let first = current_mode != last_mode;

        if first {
            match current_mode {
                ClientMode::Measure => {
                    info!("Start Measurement");
                    // Measurement signals
                    xcp_client.create_measurement_object("signal1").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal2").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal3").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal4").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal5").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal6").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal7").expect("measurement signal not found");
                    xcp_client.create_measurement_object("signal8").expect("measurement signal not found");
                    // Measure start
                    xcp_client.start_measurement().await.expect("could not start measurement");
                }

                ClientMode::Calibrate => {
                    info!("Start Calibration");
                }

                _ => {}
            }

            info!("Client mode switched from {:?} to {:?}", last_mode, current_mode);
        }

        last_mode = current_mode;

        match current_mode {
            ClientMode::Wait => {
                tokio::time::sleep(Duration::from_micros(1000)).await;
            }

            ClientMode::Measure => {
                tokio::time::sleep(Duration::from_micros(1000)).await;
            }

            ClientMode::Calibrate => {
                // Do calibrations
                const LOOPS: usize = 10000;
                let mut v = 0.0;
                let mut t: u128 = 0;
                for _ in 0..LOOPS {
                    v += 0.1;
                    trace!("CalPage.ampl = {}", v);
                    let t0 = tokio::time::Instant::now();
                    xcp_client.set_value_f64(ampl, v).await.unwrap();
                    t += t0.elapsed().as_nanos();
                }
                let t_avg = ((t / LOOPS as u128) as f64) / 1000.0; // us
                info!("Calibration performance, avg duration = {:.1} us ", t_avg);
                if t_avg > 100.0 {
                    warn!("Calibration operation duration average time exceeds 100us!")
                };
            }
            ClientMode::Stop => {
                info!("Stop");
                break;
            }
        }
    }

    // Stop measurement
    xcp_client.stop_measurement().await?;

    // Print measurement statistics
    let event_count: u64;
    let event_lost_count: u64;
    {
        let daq_decoder = daq_decoder.lock();
        event_count = daq_decoder.event_count;
        event_lost_count = daq_decoder.event_lost_count;
    }
    info!(
        "Measurement stopped, event count = {}, lost event count = {} - {:.1}%",
        event_count,
        event_lost_count,
        event_lost_count as f64 / event_count as f64 * 100.0
    );

    // Disconnect
    xcp_client.disconnect().await?;

    Ok(())
}

fn xcp_client_task(mode: Arc<parking_lot::Mutex<ClientMode>>) {
    let dest_addr: std::net::SocketAddr = "127.0.0.1:5555".parse().unwrap();
    let local_addr: std::net::SocketAddr = "0.0.0.0:0".parse().unwrap();
    info!("dest_addr: {}", dest_addr);
    info!("local_addr: {}", local_addr);

    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(xcp_client(dest_addr, local_addr, mode)).unwrap()
}

//-----------------------------------------------------------------------------

fn xcp_benchmark(c: &mut Criterion) {
    println!("XCP Benchmark");

    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();

    // Start XCP server
    let xcp = XcpBuilder::new("xcp_benchmark")
        .set_log_level(3)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
        .unwrap();

    // Create a calibration segment
    let cal_page = xcp.create_calseg("CalPage", &CAL_PAGE);
    cal_page.register_fields();

    // Measurement signal
    let mut signal1: u32 = 0;
    let mut signal2: u64 = 0;
    let mut signal3: u8 = 0;
    let mut signal4: u8 = 0;
    let mut signal5: u16 = 0;
    let mut signal6: u32 = 0;
    let mut signal7: u64 = 0;
    let mut signal8: u32 = 0;

    // Register a measurement event and bind it to the counter signal
    let event = daq_create_event!("mainloop");
    daq_register!(signal1, event);
    daq_register!(signal2, event);
    daq_register!(signal3, event);
    daq_register!(signal4, event);
    daq_register!(signal5, event);
    daq_register!(signal6, event);
    daq_register!(signal7, event);
    daq_register!(signal8, event);

    thread::sleep(Duration::from_millis(200));

    // Start XCP client task
    let mode = Arc::new(parking_lot::Mutex::new(ClientMode::Wait));
    let xcp_client_task = thread::spawn({
        let mode = mode.clone();
        move || {
            xcp_client_task(mode);
        }
    });

    thread::sleep(Duration::from_millis(200));

    // Bench deref performance
    info!("Start calibration segment deref bench");
    {
        let mut deref_bench = c.benchmark_group("calibration segment deref");

        deref_bench.bench_function("deref no sync", |b| {
            b.iter(|| {
                let _x = cal_page.ampl;
            })
        });

        deref_bench.bench_function("deref with sync", |b| {
            b.iter(|| {
                cal_page.sync();
                let _x = cal_page.ampl;
            })
        });

        deref_bench.bench_function("deref read_lock", |b| {
            b.iter(|| {
                let cal_page = cal_page.read_lock();
                let _x = cal_page.ampl;
            })
        });
    }

    // Bench calibration segment sync
    // Bench calibration operations (in xcp_client_task)
    info!("Start calibration bench");
    *mode.lock() = ClientMode::Calibrate;
    let mut count = 0;
    c.bench_function("sync", |b| {
        b.iter(|| {
            if cal_page.sync() {
                count += 1;
            }
        })
    });
    *mode.lock() = ClientMode::Wait;
    info!("Calibration bench done, changes observed: {}", count);

    thread::sleep(Duration::from_millis(200));

    // Bench measurement trigger
    signal1 += 1;
    signal2 += 1;
    signal3 += 1;
    signal4 += 1;
    signal5 += 1;
    signal6 += 1;
    signal7 += 1;
    signal8 += 1;
    info!("Start measurement bench");
    let mut count: u32 = 0;
    *mode.lock() = ClientMode::Measure;
    c.bench_function("trigger", |b| {
        b.iter(|| {
            count += 1;
            event.trigger()
        })
    });
    *mode.lock() = ClientMode::Wait;
    thread::sleep(Duration::from_millis(200));
    info!("Measurement bench done, count = {}", count);

    // Wait for stop of XCP client
    *mode.lock() = ClientMode::Stop;
    thread::sleep(Duration::from_millis(200));
    xcp_client_task.join().unwrap();
    info!("Client stopped");

    // Stop and shutdown the XCP server
    info!("Stop XCP server");
    xcp.stop_server();
    info!("Server stopped");
}

criterion_group!(benches, xcp_benchmark);
criterion_main!(benches);
