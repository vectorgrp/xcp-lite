//-----------------------------------------------------------------------------
// xcp_client is a binary crate that uses the xcp_client library crate

use std::error::Error;

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

const MAX_EVENT: usize = 16;

#[derive(Debug)]
struct DaqDecoder {
    event_count: usize,
    byte_count: usize,
    daq_timestamp: [u64; MAX_EVENT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            event_count: 0,
            byte_count: 0,
            daq_timestamp: [0; MAX_EVENT],
        }
    }
}

// Hard coded decoder for DAQ data
// This is a simple example, a real application would need to decode the data according to the actual measurement setup
// Assumes first signal is a 32 bit counter and there is only one ODT
impl XcpDaqDecoder for DaqDecoder {
    // Set start time and reset
    fn start(&mut self, timestamp: u64) {
        self.event_count = 0;
        self.byte_count = 0;
        for t in self.daq_timestamp.iter_mut() {
            *t = timestamp;
        }
    }

    // Decode DAQ data
    fn decode(&mut self, lost: u32, daq: u16, odt: u8, timestamp: u32, data: &[u8]) {
        assert!(daq < MAX_EVENT as u16);
        assert!(odt == 0);

        // Decode full 64 bit timestamp
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

        // Hardcoded:
        // Decode data of daq list 0
        // A counter:u32 assumed to be first signal in daq list 0
        if daq == 0 {
            assert!(data.len() >= 4);
            let counter = data[0] as u32 | (data[1] as u32) << 8 | (data[2] as u32) << 16 | (data[3] as u32) << 24;
            //trace!("DAQ: lost={}, daq={}, odt={}: timestamp={} counter={} data={:?}", lost, daq, odt, t, counter, data);
            info!("DAQ: lost={}, daq={}, odt={}, t={}, counter={}", lost, daq, odt, t, counter);
        }

        self.byte_count += data.len(); // overall payload byte count
        self.event_count += 1; // overall event count
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
async fn xcp_client(dest_addr: std::net::SocketAddr, local_addr: std::net::SocketAddr) -> Result<(), Box<dyn Error>> {
    println!("XCP client demo");
    println!("Calibrate and measure objects from xcp-lite main demo application");

    // Create xcp_client
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;

    // Upload A2L file
    info!("Load A2L file to file xcp_lite.a2l");
    xcp_client.load_a2l("xcp_lite.a2l", true, true).await?;

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

    // Set cycle time of main demo tasks 250ms/100us
    if let Ok(cycle_time) = xcp_client.create_calibration_object("calpage00.task1_cycle_time_us").await {
        xcp_client.set_value_u64(cycle_time, 250000).await?;
    }
    if let Ok(cycle_time) = xcp_client.create_calibration_object("calpage00.task2_cycle_time_us").await {
        xcp_client.set_value_u64(cycle_time, 100).await?;
    }

    // Measurement signals
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

    // Measurement of channel_x:f64, add all instances found in A2L file
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

    // Measure for 6 seconds
    // 32 bit DAQ timestamp will overflow after 4.2s
    let start_time = tokio::time::Instant::now();
    xcp_client.start_measurement().await?;
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    xcp_client.stop_measurement().await?;
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

    // Disconnect
    xcp_client.disconnect().await?;

    Ok(())
}

//------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log_level).init();

    let dest_addr: std::net::SocketAddr = args.dest_addr.parse().map_err(|e| format!("{}", e))?;
    let local_addr: std::net::SocketAddr = args.bind_addr.parse().map_err(|e| format!("{}", e))?;
    info!("dest_addr: {}", dest_addr);
    info!("local_addr: {}", local_addr);

    xcp_client(dest_addr, local_addr).await
}
