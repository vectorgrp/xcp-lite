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
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder { event_count: 0 }
    }
}

impl XcpDaqDecoder for DaqDecoder {
    // Handle incomming text data from XCP server
    // Hard coded decoder for DAQ data with measurement of counter:u32 or channel1/2:f64
    fn decode(&mut self, _control: &XcpTaskControl, data: &[u8]) {
        let odt = data[0];
        let _daq = data[1];
        let data_len = data.len() - 6;

        if odt == 0 {
            let timestamp = data[2] as u32
                | (data[3] as u32) << 8
                | (data[4] as u32) << 16
                | (data[5] as u32) << 24;
            if data_len == 4 {
                let counter = data[6] as u32
                    | (data[7] as u32) << 8
                    | (data[8] as u32) << 16
                    | (data[9] as u32) << 24;
                trace!(
                    "DAQ: daq={}, odt={}: timestamp={} counter={}",
                    _daq,
                    odt,
                    timestamp,
                    counter
                );
            } else if data_len == 8 {
                let b: [u8; 8] = data[6..14].try_into().unwrap();
                let f = f64::from_le_bytes(b);
                trace!(
                    "DAQ: daq={}, odt={}: timestamp={} value_f64={}",
                    _daq,
                    odt,
                    timestamp,
                    f
                );
            } else {
                trace!(
                    "DAQ: daq={}, odt={}: timestamp={} data={:?}",
                    _daq,
                    odt,
                    timestamp,
                    data
                );
            }
        } else {
            panic!("ODT != 0")
        }
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

    /// Segment size (UDP MTU)
    #[arg(short, long, default_value_t = 1472)]
    segment_size: u16,
}

//------------------------------------------------------------------------
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let log_level = args.log_level.to_log_level_filter();

    env_logger::Builder::new().filter_level(log_level).init();

    // Create xcp_client
    let dest_addr: Result<SocketAddr, _> = args.dest_addr.parse();
    let local_addr: Result<SocketAddr, _> = args.bind_addr.parse();
    info!("dest_addr: {:?}", dest_addr);
    info!("local_addr: {:?}", local_addr);
    let mut xcp_client = XcpClient::new(dest_addr.unwrap(), local_addr.unwrap());

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    let res = xcp_client
        .connect(Arc::clone(&daq_decoder), ServTextDecoder::new())
        .await;
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
    if let Ok(counter_max) = xcp_client
        .create_calibration_object("CalPage1.counter_max")
        .await
    {
        // Get current value
        let v = xcp_client.get_value_u64(counter_max);
        info!("CalPage1.counter_max = {}", v);

        // Set value to 1000
        info!("CalPage1.counter_max = {}", v);
        xcp_client.set_value_u64(counter_max, 256).await.unwrap();
    }

    info!("XCP Measurement");
    // Measurement of counter:u32
    xcp_client.create_measurement_object("counter").unwrap();
    info!(r#"Created measurement object "counter""#);
    // if let Some(_m) = xcp_client.create_measurement_object("counter_u8") {
    //     info!(r#"Created measurement object counter_u8"#);
    // }
    // if let Some(_m) = xcp_client.create_measurement_object("counter_u16") {
    //     info!(r#"Created measurement object counter_u16"#);
    // }
    // if let Some(_m) = xcp_client.create_measurement_object("counter_u32") {
    //     info!(r#"Created measurement object counter_u32"#);
    // }
    // if let Some(_m) = xcp_client.create_measurement_object("counter_u64") {
    //     info!(r#"Created measurement object counter_u64"#);
    // }

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
    xcp_client.start_measurement().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    xcp_client.stop_measurement().await.unwrap();
    let event_count = daq_decoder.lock().unwrap().event_count;
    info!("Measurement done, {} events", event_count);
    assert_ne!(event_count, 0);

    // Stop demo task
    // Create a calibration object for CalPage1.counter_max
    /*
        if let Ok(run) = xcp_client.create_calibration_object("CalPage.run").await {
            let v = xcp_client.get_value_u64(run);
            info!("CalPage.run = {}", v);
            assert_eq!(v, 1);
            xcp_client.set_value_u64(run, 0).await.unwrap();
        }
    */

    // Disconnect
    info!("XCP Disconnect");
    xcp_client.disconnect().await.unwrap();
}
