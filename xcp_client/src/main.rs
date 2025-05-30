//-----------------------------------------------------------------------------
// xcp_client - XCP client example
// This example demonstrates how to connect to an XCP server, load an A2L file, read and write calibration variables,
// and measure data using the DAQ protocol.
// The example uses the tokio runtime and async/await syntax.
//
// Run:
// cargo r --example xcp_client -- -h

use parking_lot::Mutex;
use std::{error::Error, sync::Arc};

mod xcp_client;
use xcp_client::*;

mod xcp_test_executor;
use xcp_test_executor::test_executor;

//-----------------------------------------------------------------------------
// Command line arguments

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // -l --log-level
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = 3)]
    log_level: u8,

    // -d --dest_addr
    /// XCP server address
    #[arg(short, long, default_value = "127.0.0.1:5555")]
    dest_addr: String,

    // -p --port
    /// XCP server port number
    #[arg(short, long, default_value_t = 5555)]
    port: u16,

    // -b -- bind-addr
    /// Bind address, master port number
    #[arg(short, long, default_value = "0.0.0.0:9999")]
    bind_addr: String,

    // --list_mea
    /// Lists all matchin measurement variables found in the A2L file
    #[clap(long, default_value = "")]
    list_mea: String,

    // --list-cal
    /// Lists all matching calibration variables found in the A2L file
    #[clap(long, default_value = "")]
    list_cal: String,

    // -m --mea
    /// Specify variable names for DAQ measurement, may be list of names separated by space or single regular expressions (e.g. ".*")
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    mea: Vec<String>,

    // -t --time
    /// Specify measurement duration in ms
    #[arg(short, long, default_value_t = 5000)]
    time_ms: u64, // -t --time

    /// --test
    #[arg(long, default_value_t = false)]
    test: bool,
}

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

//-----------------------------------------------------------------------------
// Test (--test) settings

const TEST_CAL: xcp_test_executor::TestModeCal = xcp_test_executor::TestModeCal::Cal; // Execute calibration tests: Cal or None
const TEST_DAQ: xcp_test_executor::TestModeDaq = xcp_test_executor::TestModeDaq::Daq; // Execute measurement tests: Daq or None
const TEST_DURATION_MS: u64 = 5000;

//------------------------------------------------------------------------
// Demo

const MAX_EVENT: usize = 16;

// DaqDecoder for xcp_client_demo - handle incoming DAQ data
// This is a simple example of a DAQ decoder that prints the decoded data to the console
// It can be used as a template for more advanced DAQ decoders

#[derive(Debug)]
struct DaqDecoder {
    daq_odt_entries: Option<Vec<Vec<OdtEntry>>>,
    timestamp_resolution: u64,
    daq_header_size: u8,
    event_count: usize,
    byte_count: usize,
    daq_timestamp: [u64; MAX_EVENT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            daq_odt_entries: None,
            timestamp_resolution: 0,
            daq_header_size: 0,
            event_count: 0,
            byte_count: 0,
            daq_timestamp: [0; MAX_EVENT],
        }
    }
}

// Decoder for DAQ data
impl XcpDaqDecoder for DaqDecoder {
    // Set start time and init
    fn start(&mut self, daq_odt_entries: Vec<Vec<OdtEntry>>, timestamp: u64) {
        // Init
        self.daq_odt_entries = Some(daq_odt_entries);
        self.event_count = 0;
        self.byte_count = 0;
        for t in self.daq_timestamp.iter_mut() {
            *t = timestamp;
        }
    }

    fn stop(&mut self) {}

    // Set timestamp resolution
    fn set_daq_properties(&mut self, timestamp_resolution: u64, daq_header_size: u8) {
        self.daq_header_size = daq_header_size;
        self.timestamp_resolution = timestamp_resolution;
    }

    // Decode DAQ data
    fn decode(&mut self, lost: u32, buf: &[u8]) {
        let daq: u16;
        let odt: u8;
        let mut timestamp_raw: u32 = 0;
        let data: &[u8];

        // Decode header and raw timestamp
        if self.daq_header_size == 4 {
            daq = (buf[2] as u16) | ((buf[3] as u16) << 8);
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = (buf[4] as u32) | ((buf[4 + 1] as u32) << 8) | ((buf[4 + 2] as u32) << 16) | ((buf[4 + 3] as u32) << 24);
                data = &buf[8..];
            } else {
                data = &buf[4..];
            }
        } else {
            daq = buf[1] as u16;
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = (buf[2] as u32) | ((buf[2 + 1] as u32) << 8) | ((buf[2 + 2] as u32) << 16) | ((buf[2 + 3] as u32) << 24);
                data = &buf[6..];
            } else {
                data = &buf[2..];
            }
        }

        assert!(daq < MAX_EVENT as u16);
        assert!(odt == 0);

        // Decode full 64 bit daq timestamp
        let t_last = self.daq_timestamp[daq as usize];
        let t: u64 = if odt == 0 {
            let tl = (t_last & 0xFFFFFFFF) as u32;
            let mut th = (t_last >> 32) as u32;
            if timestamp_raw < tl {
                th += 1;
            }
            let t = (timestamp_raw as u64) | ((th as u64) << 32);
            if t < t_last {
                warn!("Timestamp of daq {} declining {} -> {}", daq, t_last, t);
            }
            self.daq_timestamp[daq as usize] = t;
            t
        } else {
            t_last
        };

        println!("DAQ: lost={}, daq={}, odt={}, t={}ns (+{}us)", lost, daq, odt, t, (t - t_last) / 1000);

        // Get daq list
        let daq_list = &self.daq_odt_entries.as_ref().unwrap()[daq as usize];

        // Decode all odt entries
        for odt_entry in daq_list.iter() {
            let value_size = odt_entry.a2l_type.size;
            let mut value_offset = odt_entry.offset as usize + value_size - 1;
            let mut value: u64 = 0;
            loop {
                value |= data[value_offset] as u64;
                if value_offset == odt_entry.offset as usize {
                    break;
                };
                value <<= 8;
                value_offset -= 1;
            }
            match odt_entry.a2l_type.encoding {
                A2lTypeEncoding::Signed => {
                    match value_size {
                        1 => {
                            let signed_value: i8 = value as u8 as i8;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        2 => {
                            let signed_value: i16 = value as u16 as i16;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        4 => {
                            let signed_value: i32 = value as u32 as i32;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        8 => {
                            let signed_value: i64 = value as i64;
                            println!(" {} = {}", odt_entry.name, signed_value);
                        }
                        _ => {
                            warn!("Unsupported signed value size {}", value_size);
                        }
                    };
                }
                A2lTypeEncoding::Unsigned => {
                    println!(" {} = {}", odt_entry.name, value);
                }
                A2lTypeEncoding::Float => {
                    if odt_entry.a2l_type.size == 4 {
                        #[allow(clippy::transmute_int_to_float)]
                        let value: f32 = unsafe { std::mem::transmute(value as u32) };
                        println!(" {} = {}", odt_entry.name, value);
                    } else {
                        #[allow(clippy::transmute_int_to_float)]
                        let value: f64 = unsafe { std::mem::transmute(value) };
                        println!(" {} = {}", odt_entry.name, value);
                    }
                }
                A2lTypeEncoding::Blob => {
                    panic!("Blob not supported");
                }
            }
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
        print!("[SERV_TEXT] ");
        let mut j = 0;
        while j < data.len() {
            print!("{}", data[j] as char);
            j += 1;
        }
    }
}

//------------------------------------------------------------------------
// A simple example how to use the XCP client

async fn xcp_client_demo(
    dest_addr: std::net::SocketAddr,
    local_addr: std::net::SocketAddr,
    list_cal: String,
    list_mea: String,
    measurement_list: Vec<String>,
    measurement_time_ms: u64,
) -> Result<(), Box<dyn Error>> {
    // Create xcp_client
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;

    // Upload A2L file
    info!("Load A2L file");
    xcp_client.a2l_loader().await?;

    // Print all known calibration objects and get their current value
    if !list_cal.is_empty() {
        println!();
        println!("Calibration variables:");
        let cal_objects = xcp_client.find_characteristics(list_cal.as_str());
        for name in &cal_objects {
            let h: XcpCalibrationObjectHandle = xcp_client.create_calibration_object(name).await?;
            match xcp_client.get_calibration_object(h).get_a2l_type().encoding {
                A2lTypeEncoding::Signed => {
                    let v = xcp_client.get_value_i64(h);
                    let o = xcp_client.get_calibration_object(h);
                    println!(" {} {}:{:08X} = {}", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr, v);
                }
                A2lTypeEncoding::Unsigned => {
                    let v = xcp_client.get_value_u64(h);
                    let o = xcp_client.get_calibration_object(h);
                    println!(" {} {}:{:08X} = {}", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr, v);
                }
                A2lTypeEncoding::Float => {
                    let v = xcp_client.get_value_f64(h);
                    let o = xcp_client.get_calibration_object(h);
                    println!(" {} {}:{:08X} = {}", o.get_name(), o.get_a2l_addr().ext, o.get_a2l_addr().addr, v);
                }
                A2lTypeEncoding::Blob => {
                    println!(" {} = [...]", name);
                }
            }
        }
        println!();
        return Ok(());
    }

    // Print all known measurement objects
    if !list_mea.is_empty() {
        println!();
        println!("Measurement variables:");
        let mea_objects = xcp_client.find_measurements(&list_mea);
        for name in &mea_objects {
            if let Some(h) = xcp_client.create_measurement_object(name) {
                let o = xcp_client.get_measurement_object(h);
                println!(" {} {} {}", o.get_name(), o.get_a2l_addr(), o.get_a2l_type());
            }
        }
        println!();
        return Ok(());
    }

    // Measurement
    if !measurement_list.is_empty() {
        let list = if measurement_list.len() == 1 {
            // Regular expression
            xcp_client.find_measurements(measurement_list[0].as_str())
        } else {
            // Just a list of names
            measurement_list
        };

        // Create measurement objects for all names in the lis
        // Multi dimensional objects not supported yet
        info!("Measurement list:");
        for name in &list {
            if let Some(o) = xcp_client.create_measurement_object(name) {
                info!(r#"  {}: {}"#, o.0, name);
            }
        }
        info!("");

        // Measure for n seconds
        // 32 bit DAQ timestamp will overflow after 4.2s
        let start_time = tokio::time::Instant::now();
        xcp_client.start_measurement().await?;
        tokio::time::sleep(std::time::Duration::from_millis(measurement_time_ms)).await;
        xcp_client.stop_measurement().await?;
        let elapsed_time = start_time.elapsed().as_micros();

        // Print statistics from DAQ decoder
        let event_count = daq_decoder.lock().event_count;
        let byte_count = daq_decoder.lock().byte_count;
        info!(
            "Measurement done, {} events, {:.0} event/s, {:.3} Mbytes/s",
            event_count,
            event_count as f64 * 1_000_000.0 / elapsed_time as f64,
            byte_count as f64 / elapsed_time as f64
        );
    }

    // Disconnect
    xcp_client.disconnect().await?;

    Ok(())
}

//------------------------------------------------------------------------
// Main function

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();

    let dest_addr: std::net::SocketAddr = args.dest_addr.parse().map_err(|e| format!("{}", e))?;
    let local_addr: std::net::SocketAddr = args.bind_addr.parse().map_err(|e| format!("{}", e))?;
    info!("dest_addr: {}", dest_addr);
    info!("local_addr: {}", local_addr);

    // Run the test executor if --test is specified
    if args.test {
        test_executor(dest_addr, local_addr, TEST_CAL, TEST_DAQ, TEST_DURATION_MS).await; // Start the test executor
        Ok(())
    } else {
        // Measurement variable list from command line
        let measurement_list = args.mea;
        if !measurement_list.is_empty() {
            info!("measurement_list: {:?}", measurement_list);
        }

        // Start XCP client

        xcp_client_demo(dest_addr, local_addr, args.list_cal, args.list_mea, measurement_list, args.time_ms).await
    }
}
