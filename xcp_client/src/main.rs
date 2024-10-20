//-----------------------------------------------------------------------------
// xcp_client is a binary crate that uses the xcp_client library crate

//use ::xcp_client::a2l::a2l_reader::A2lTypeEncoding;
use a2l::a2l_reader::A2lTypeEncoding;
use parking_lot::Mutex;
use std::{collections::HashMap, error::Error, sync::Arc};
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
    odt_entries: Option<Arc<Mutex<HashMap<String, OdtEntry>>>>,
    timestamp_resolution: u64,
    daq_header_size: u8,
    event_count: usize,
    byte_count: usize,
    daq_timestamp: [u64; MAX_EVENT],
}

impl DaqDecoder {
    pub fn new() -> DaqDecoder {
        DaqDecoder {
            odt_entries: None,
            timestamp_resolution: 0,
            daq_header_size: 0,
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
    fn start(&mut self, odt_entries: Arc<Mutex<HashMap<String, OdtEntry>>>, timestamp: u64) {
        self.odt_entries = Some(odt_entries);
        self.event_count = 0;
        self.byte_count = 0;
        for t in self.daq_timestamp.iter_mut() {
            *t = timestamp;
        }
    }

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
            daq = buf[2] as u16 | (buf[3] as u16) << 8;
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = buf[4] as u32 | (buf[4 + 1] as u32) << 8 | (buf[4 + 2] as u32) << 16 | (buf[4 + 3] as u32) << 24;
                data = &buf[8..];
            } else {
                data = &buf[4..];
            }
        } else {
            daq = buf[1] as u16;
            odt = buf[0];
            if odt == 0 {
                timestamp_raw = buf[2] as u32 | (buf[2 + 1] as u32) << 8 | (buf[2 + 2] as u32) << 16 | (buf[2 + 3] as u32) << 24;
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
            let t = timestamp_raw as u64 | (th as u64) << 32;
            if t < t_last {
                warn!("Timestamp of daq {} declining {} -> {}", daq, t_last, t);
            }
            self.daq_timestamp[daq as usize] = t;
            t
        } else {
            t_last
        };

        // Decode all odt entries
        println!("DAQ: lost={}, daq={}, odt={}, t={}ns", lost, daq, odt, t);
        if let Some(o) = self.odt_entries.as_ref() {
            for e in o.lock().iter() {
                let odt_entry: &OdtEntry = e.1;
                if odt_entry.daq == daq && odt_entry.odt == odt {
                    let value_size = odt_entry.a2l_type.size as usize;
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
                                    println!("{}:  {} = {}", t, e.0, signed_value);
                                }
                                2 => {
                                    let signed_value: i16 = value as u16 as i16;
                                    println!("{}:  {} = {}", t, e.0, signed_value);
                                }
                                4 => {
                                    let signed_value: i32 = value as u32 as i32;
                                    println!("{}:  {} = {}", t, e.0, signed_value);
                                }
                                8 => {
                                    let signed_value: i64 = value as i64;
                                    println!("{}:  {} = {}", t, e.0, signed_value);
                                }
                                _ => {
                                    warn!("Unsupported signed value size {}", value_size);
                                }
                            };
                        }
                        A2lTypeEncoding::Unsigned => {
                            println!("{}:  {} = {}", t, e.0, value);
                        }
                        A2lTypeEncoding::Float => {
                            if odt_entry.a2l_type.size == 4 {
                                #[allow(clippy::transmute_int_to_float)]
                                let value: f32 = unsafe { std::mem::transmute(value as u32) };
                                println!("{}:  {} = {}", t, e.0, value);
                            } else {
                                #[allow(clippy::transmute_int_to_float)]
                                let value: f64 = unsafe { std::mem::transmute(value) };
                                println!("{}:  {} = {}", t, e.0, value);
                            }
                        }
                    }
                }
            }
        }

        // Hardcoded:
        // Decode data of daq list 0
        // A counter:u32 assumed to be first signal in daq list 0
        // if daq == 0 {
        //     assert!(data.len() >= 4);
        //     let counter = data[0] as u32 | (data[1] as u32) << 8 | (data[2] as u32) << 16 | (data[3] as u32) << 24;
        //     //trace!("DAQ: lost={}, daq={}, odt={}: timestamp={} counter={} data={:?}", lost, daq, odt, t, counter, data);
        //     let t = t * self.timestamp_resolution;
        //     info!("DAQ: lost={}, daq={}, odt={}, t={}ns, counter={}", lost, daq, odt, t, counter);
        // }

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
    #[arg(short, long, default_value_t = 2)]
    log_level: u8,

    /// XCP server address
    #[arg(short, long, default_value = "127.0.0.1:5555")]
    dest_addr: String,

    /// XCP server port number
    #[arg(short, long, default_value_t = 5555)]
    port: u16,

    /// Bind address, master port number
    #[arg(short, long, default_value = "0.0.0.0:9999")]
    bind_addr: String,

    /// Print detailled A2L infos
    #[clap(long)]
    print_a2l: bool,

    /// Lists all measurement variables
    #[clap(long)]
    list_mea: bool,

    /// Lists all calibration variables
    #[clap(long)]
    list_cal: bool,

    /// Specifies the variables names for DAQ measurement, 'all' or a list of names separated by space
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    measurement_list: Vec<String>,

    /// A2L filename, default is upload A2L file
    #[arg(short, long)]
    a2l_filename: Option<String>,
}

//------------------------------------------------------------------------
async fn xcp_client(
    dest_addr: std::net::SocketAddr,
    local_addr: std::net::SocketAddr,
    a2l_filename: Option<String>,
    print_a2l: bool,
    list_cal: bool,
    list_mea: bool,
    measurement_list: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    // Create xcp_client
    let mut xcp_client = XcpClient::new(dest_addr, local_addr);

    // Connect to the XCP server
    info!("XCP Connect");
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;

    // Upload A2L file
    info!("Load A2L file");
    xcp_client.a2l_loader(a2l_filename, print_a2l).await?;

    // Print all calibration objects with current value
    if list_cal {
        println!();
        println!("Calibration variables:");
        let cal_objects = xcp_client.get_characteristics();
        for name in cal_objects.iter() {
            let h = xcp_client.create_calibration_object(name).await?;
            let o = xcp_client.get_calibration_object(h);

            match o.get_type().encoding {
                A2lTypeEncoding::Signed => {
                    let v = xcp_client.get_value_i64(h);
                    println!(" {} = {}", name, v);
                }
                A2lTypeEncoding::Unsigned => {
                    let v = xcp_client.get_value_u64(h);
                    println!(" {} = {}", name, v);
                }
                A2lTypeEncoding::Float => {
                    let v = xcp_client.get_value_f64(h);
                    println!(" {} = {:.8}", name, v);
                }
            }
        }
        println!();
    }

    // Print all measurement objects
    if list_mea {
        println!();
        println!("Measurement variables:");
        let mea_objects = xcp_client.get_measurements();
        for name in mea_objects.iter() {
            println!(" {}", name);
        }
        println!();
    }

    // Calibration
    // Change the value of CalPage1.counter_max to 255 (if exists - from main.rs, hello_xcp.rs, multi_thread_demo.rs)
    // Measure how long this takes
    let start_time = tokio::time::Instant::now();
    if let Ok(counter_max) = xcp_client.create_calibration_object("CalPage1.counter_max").await {
        let v = xcp_client.get_value_u64(counter_max);
        let elapsed_time_1 = start_time.elapsed().as_micros();
        xcp_client.set_value_u64(counter_max, 255).await.unwrap();
        let elapsed_time_2 = start_time.elapsed().as_micros();
        info!("Get CalPage1.counter_max = {} (duration = {}us)", v, elapsed_time_1);
        info!("Set CalPage1.counter_max to {} (duration = {}us)", 255, elapsed_time_2);
    }
    // Change the value of ampl to 100.0 (if exists - from XCPlite)
    // Measure how long this takes
    let start_time = tokio::time::Instant::now();
    if let Ok(counter_max) = xcp_client.create_calibration_object("ampl").await {
        let v = xcp_client.get_value_f64(counter_max);
        let elapsed_time_1 = start_time.elapsed().as_micros();
        xcp_client.set_value_f64(counter_max, 123.0).await.unwrap();
        let elapsed_time_2 = start_time.elapsed().as_micros();
        info!("Get ampl = {} (duration = {}us)", v, elapsed_time_1);
        info!("Set ampl to {} (duration = {}us)", 123.0, elapsed_time_2);
    }

    // Measure
    let measure_all: bool = measurement_list.len() == 1 && measurement_list[0] == "all";

    if measurement_list.len() > 0 || measure_all {
        // Set cycle time of main demo tasks 250ms/100us (if exists - from main.rs)
        // counter_x task 1 cycle time
        if let Ok(cycle_time) = xcp_client.create_calibration_object("static_cal_page.task1_cycle_time_us").await {
            xcp_client.set_value_u64(cycle_time, 1000).await?;
        }
        // channel_x task 2 cycle time
        if let Ok(cycle_time) = xcp_client.create_calibration_object("static_cal_page.task2_cycle_time_us").await {
            xcp_client.set_value_u64(cycle_time, 100000).await?;
        }
        info!("");

        // Measure all existing measurement variables or the list of variables provided
        // Multi dimensional objects not supported yet
        info!("Measurement variables");
        let mea_objects = if !measure_all { measurement_list } else { xcp_client.get_measurements() };
        for o in mea_objects.iter() {
            if let Some(_m) = xcp_client.create_measurement_object(o) {
                info!(r#"  Created measurement object {}"#, o);
            }
        }
        info!("");

        // Measure for 6 seconds
        // 32 bit DAQ timestamp will overflow after 4.2s
        let start_time = tokio::time::Instant::now();
        xcp_client.start_measurement().await?;
        tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        xcp_client.stop_measurement().await?;
        let elapsed_time = start_time.elapsed().as_micros();

        // Print statistics
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
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log_level).init();

    let dest_addr: std::net::SocketAddr = args.dest_addr.parse().map_err(|e| format!("{}", e))?;
    let local_addr: std::net::SocketAddr = args.bind_addr.parse().map_err(|e| format!("{}", e))?;
    info!("dest_addr: {}", dest_addr);
    info!("local_addr: {}", local_addr);

    let measurement_list = args.measurement_list;
    if measurement_list.len() > 0 {
        info!("measurement_list: {:?}", measurement_list);
    }

    if args.a2l_filename.is_some() {
        info!("a2l_filename: {}", args.a2l_filename.as_ref().unwrap());
    }

    xcp_client(dest_addr, local_addr, args.a2l_filename, args.print_a2l, args.list_cal, args.list_mea, measurement_list).await
}
