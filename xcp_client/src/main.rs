//-----------------------------------------------------------------------------
// xcp_client is a binary crate that uses the xcp_client library crate

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

mod xcp_client;
use xcp_client::*;
mod a2l;

//----------------------------------------------------------------------------------------------
// Logging

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

trait ToLogLevelFilter {
    fn to_log_level_filter(self) -> log::LevelFilter;
}

impl ToLogLevelFilter for u8 {
    fn to_log_level_filter(self) -> log::LevelFilter {
        match self {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Warn,
        }
    }
}

//------------------------------------------------------------------------
// Handle incomming DAQ data

#[derive(Debug, Clone, Copy)]
struct DaqDecoder {
    // Add any state needed to decode DAQ data
    event_count: usize,
    byte_count: usize,
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder { event_count: 0, byte_count: 0 }
    }
}

// Hard coded decoder for DAQ data
// This is a simple example, a real application would need to decode the data according to the actual measurement setup
// Assumes first signal is a 32 bit counter and there is only one ODT
impl XcpDaqDecoder for DaqDecoder {
    // Handle incomming text data from XCP server
    // Hard coded decoder for DAQ data with measurement of counter:u32 or channel_x:f64
    fn decode(&mut self, _control: &XcpTaskControl, data: &[u8]) {
        let odt = data[0];
        let daq = data[1];
        let data_len = data.len() - 6; // 6 bytes header (odt, daq, timestamp)

        if odt == 0 {
            assert!(data_len >= 4);
            let timestamp = data[2] as u32 | (data[3] as u32) << 8 | (data[4] as u32) << 16 | (data[5] as u32) << 24;
            let counter = data[6] as u32 | (data[7] as u32) << 8 | (data[8] as u32) << 16 | (data[9] as u32) << 24;
            trace!("DAQ: daq={}, odt={}: timestamp={} counter={} data={:?}", daq, odt, timestamp, counter, data);
        } else {
            panic!("ODT != 0")
        }

        self.byte_count += data_len; // payload byte count
        self.event_count += 1;
    }
}

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

//-----------------------------------------------------------------------------
// Command line arguments

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = 3)]
    log_level: u8,

    /// Server address
    #[arg(short, long, default_value = "127.0.0.1:5555")]
    dest_addr: String,

    /// Port number
    #[arg(short, long, default_value_t = 5555)]
    port: u16,

    /// Bind address
    #[arg(short, long, default_value = "0.0.0.0:0")]
    bind_addr: String,
}

//------------------------------------------------------------------------
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new().filter_level(log_level).init();

    println!("Test XCP client demo application");
    println!("Calibrate and measure objects from xcp-lite main demo application");
    println!("Measure counter from task1 and all channel_x from all task2 instances");
    println!("Calibrate the task cycle time and counter_max");

    // Create xcp_client
    let dest_addr: Result<SocketAddr, _> = args.dest_addr.parse();
    let local_addr: Result<SocketAddr, _> = args.bind_addr.parse();
    info!("dest_addr: {:?}", dest_addr);
    info!("local_addr: {:?}", local_addr);
    let mut xcp_client = XcpClient::new(dest_addr.unwrap(), local_addr.unwrap());

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    let res = xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await;
    match res {
        Ok(_) => info!("Connected!"),
        Err(e) => {
            e.downcast_ref::<XcpError>()
                .map(|e| {
                    error!("XCP error: {}", e);
                })
                .or_else(|| panic!("connect failed!"));

            return;
        }
    }

    // Upload A2L file
    info!("XCP Upload A2L");
    xcp_client.upload_a2l().await.unwrap();

    // Calibration
    info!("XCP calibration");
    // Create a calibration object for CalPage1.counter_max
    let start_time = tokio::time::Instant::now();
    if let Ok(counter_max) = xcp_client.create_calibration_object("CalPage1.counter_max").await {
        let v = xcp_client.get_value_u64(counter_max);
        let elapsed_time_1 = start_time.elapsed().as_micros();
        xcp_client.set_value_u64(counter_max, 255).await.unwrap();
        let elapsed_time_2 = start_time.elapsed().as_micros();
        info!("Get CalPage1.counter_max = {} (duration = {}us)", v, elapsed_time_1);
        info!("Set CalPage1.counter_max to {} (duration = {}us)", 255, elapsed_time_2);
    } else {
        warn!("CalPage1.counter_max not found");
    }

    info!("XCP Measurement");

    // Speed up cycle time of demo tasks to 50us
    if let Ok(cycle_time) = xcp_client.create_calibration_object("calpage00.task1_cycle_time_us").await {
        xcp_client.set_value_u64(cycle_time, 50).await.unwrap();
    }
    if let Ok(cycle_time) = xcp_client.create_calibration_object("calpage00.task2_cycle_time_us").await {
        xcp_client.set_value_u64(cycle_time, 50).await.unwrap();
    }

    // Measurement of counter:u32
    xcp_client.create_measurement_object("counter").unwrap();
    info!(r#"Created measurement object "counter""#);
    if let Some(_m) = xcp_client.create_measurement_object("counter_u8") {
        info!(r#"Created measurement object counter_u8"#);
    }
    if let Some(_m) = xcp_client.create_measurement_object("counter_u16") {
        info!(r#"Created measurement object counter_u16"#);
    }
    if let Some(_m) = xcp_client.create_measurement_object("counter_u32") {
        info!(r#"Created measurement object counter_u32"#);
    }
    if let Some(_m) = xcp_client.create_measurement_object("counter_u64") {
        info!(r#"Created measurement object counter_u64"#);
    }

    // Measurement of channel_x:f64, add all instances found
    let mut i = 0;
    loop {
        i += 1;
        let name = format!("channel_{}", i);
        if let Some(_m) = xcp_client.create_measurement_object(name.as_str()) {
            info!(r#"Created measurement object "{}""#, name.as_str());
        } else {
            break;
        };
    }

    // Measure for 2 seconds
    let start_time = tokio::time::Instant::now();
    xcp_client.start_measurement().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    xcp_client.stop_measurement().await.unwrap_or_else(|e| error!("Stop measurement failed: {}", e));
    let elapsed_time = start_time.elapsed().as_micros();

    // Print statistics
    let event_count = daq_decoder.lock().unwrap().event_count;
    let byte_count = daq_decoder.lock().unwrap().byte_count;
    info!(
        "Measurement done, {} events, {:.0} event/s, {:.3} Mbytes/s",
        event_count,
        event_count as f64 * 1_000_000.0 / elapsed_time as f64,
        byte_count as f64 / elapsed_time as f64
    );
    info!("Expected {} events/s, {} byte/s", 1_000_000 / 50 * 10, 90 * 1_000_000 / 50 * 10);

    // Disconnect
    xcp_client.disconnect().await.unwrap();
}

// Start/Stop demo task
// Create a calibration object for CalPage.run
// impl XcpClient {
//     async fn set_run(&mut self, state: bool) -> Result<(), XcpError> {
//         if let Ok(run) = self.create_calibration_object("CalPage.run").await {
//             let v = self.get_value_u64(run);
//             info!("CalPage.run = {}", v);
//             assert_eq!(v, 1);
//             self.set_value_u64(run, state as u64).await.unwrap();
//         } else {
//             warn!("CalPage.run not found");
//         }

//         Ok(())
//     }
// }
