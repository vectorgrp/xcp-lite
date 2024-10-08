// cargo bench -- --save-baseline parking_lot
// cargo bench -- --baseline parking_lot
// cargo bench -- --save-baseline parking_lot
// cargo bench -- --load-baseline new --baseline parking_lot
// --warm-up-time 0
// --nresamples <nresamples>
//
//

#![allow(unused_assignments)]
#![allow(unused_imports)]

use log::{debug, error, info, trace, warn};

use std::{
    fmt::Debug,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use xcp::*;
use xcp_client::xcp_client::*;
use xcp_type_description::prelude::*;

use criterion::{criterion_group, criterion_main, Criterion};
//-----------------------------------------------------------------------------
// Calibration parameters

#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
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
        DaqDecoder { event_count: 0, event_lost_count: 0 }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    fn decode(&mut self, lost: u32, _daq: u16, _odt: u8, _time: u32, _data: &[u8]) {
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
    info!("Load A2L file to file xcp_lite.a2l");
    xcp_client.load_a2l("xcp_lite.a2l", true, true).await?;

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
    let event_count = daq_decoder.lock().unwrap().event_count;
    let event_lost_count = daq_decoder.lock().unwrap().event_lost_count;
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

    env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();

    // Start XCP server
    let xcp = XcpBuilder::new("xcp_benchmark")
        .set_log_level(XcpLogLevel::Info)
        .set_epk("EPK_")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)
        .unwrap();

    // Create a calibration segment
    let cal_page = xcp.create_calseg("CalPage", &CAL_PAGE);

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

    // Wait a moment
    thread::sleep(Duration::from_millis(100));

    // Start XCP client task
    let mode = Arc::new(parking_lot::Mutex::new(ClientMode::Wait));
    let mode_cloned = mode.clone();
    let xcp_client_task = thread::spawn(move || {
        xcp_client_task(mode_cloned);
    });

    // Wait a moment
    thread::sleep(Duration::from_millis(100));

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

    // Wait a moment
    thread::sleep(Duration::from_millis(100));

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
    thread::sleep(Duration::from_millis(100));
    info!("Measurement bench done, count = {}", count);

    // Wait a moment
    thread::sleep(Duration::from_millis(100));

    // Wait for stop of XCP client
    *mode.lock() = ClientMode::Stop;
    xcp_client_task.join().unwrap();
    info!("Client stopped");

    // Stop XCP server
    xcp.stop_server();
    info!("Server stopped");
}

criterion_group!(benches, xcp_benchmark);
criterion_main!(benches);
