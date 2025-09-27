//-----------------------------------------------------------------------------
// xcp_client - XCP client example
// This example demonstrates how to connect to an XCP server, load an A2L file, read and write calibration variables,
// and measure data using the DAQ protocol.
// The example uses the tokio runtime and async/await syntax.
//
// Run:
// cargo r --example xcp_client -- -h

use parking_lot::Mutex;
use std::net::Ipv4Addr;
use std::{error::Error, sync::Arc};
use xcp_lite::registry::{McEvent};

// External crates for ELF parsing
//use goblin;

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

    // -b --bind-addr
    /// Bind address, master port number
    #[arg(short, long, default_value = "0.0.0.0:9999")]
    bind_addr: String,

    // --tcp
    /// Use TCP instead of UDP for XCP communication
    #[arg(long, default_value_t = false)]
    tcp: bool,

    // -a, --a2l
    /// Specify the name for the A2L file
    #[arg(short, long, default_value = "xcp_client")]
    a2l: String,

    // -e, --elf
    /// Specify the name of the ELF file
    #[arg(short, long, default_value = "")]
    elf: String,

    // --load-a2l
    /// Load A2L file from XCP server
    /// Requires that the XCP server supports the A2L upload command
    #[arg(long, default_value_t = false)]
    load_a2l: bool,

    // --list_mea
    /// Lists all matching measurement variables found in the A2L file
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

    // --cal
    /// Set calibration variable to a value (format: "variable_name value")
    #[clap(long, value_names = ["NAME", "VALUE"], num_args = 2)]
    cal: Vec<String>,

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
// Handle incoming DAQ data
// Prints the decoded data to the console

const MAX_EVENT: usize = 64;

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
// Handle incoming SERV_TEXT data
// Prints the text to the console

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
//  Binary reader (ELF and Mach-O)

/*
fn read_elf(reg: &mut Registry, file_name: &str) -> Result<(), Box<dyn Error>> {
    use std::fs::read;

    println!("Starting binary analysis of: {}", file_name);

    // Read and parse binary file
    let buffer = read(file_name).map_err(|e| format!("Failed to read binary file '{}': {}", file_name, e))?;
    println!("Successfully read {} bytes from binary file", buffer.len());

    // Parse the object file (auto-detect format)
    let object = goblin::Object::parse(&buffer).map_err(|e| format!("Failed to parse binary file: {}", e))?;
    match object {
        goblin::Object::Elf(elf) => {
            println!("Detected ELF format");
            process_elf_file(reg, &elf)?;
        }
        _ => {
            println!("Unsupported object format");
            return Err(format!("Unsupported object format").into());
        }
    }

    Ok(())
}

fn process_elf_file(reg: &mut Registry, elf: &goblin::elf::Elf) -> Result<(), Box<dyn std::error::Error>> {

    // Extract only global variable symbols (exclude functions and local variables)
    for sym in elf.syms.iter() {
        let bind = sym.st_bind();
        let typ = sym.st_type();
        let sec = sym.st_shndx;
        let name = elf.strtab.get_at(sym.st_name).unwrap_or("").trim_end_matches('\0');

        // Only show GLOBAL object symbols (variables)
        // Exclude functions (STT_FUNC) and local variables (STB_LOCAL)
        if typ == goblin::elf::sym::STT_OBJECT
            && bind == goblin::elf::sym::STB_GLOBAL
            && sec != 0
            && !name.is_empty()
            && !name.starts_with("__")  // Exclude compiler-generated symbols
            && !name.starts_with("_")   // Exclude private/system symbols
        {
            let addr = sym.st_value;
            let size = sym.st_size;

            println!("Global Variable - Address: 0x{:08x}, Size: {}, Name: {}", addr, size, name);
            let mc_support_data = McSupportData::new(McObjectType::Measurement);
            let dim_type = match size {
                1 => McDimType::new(McValueType::Ubyte, 0, 0),
                2 => McDimType::new(McValueType::Uword, 0, 0),
                4 => McDimType::new(McValueType::Ulong, 0, 0),
                8 => McDimType::new(McValueType::Ulonglong, 0, 0),
                _ => McDimType::new(McValueType::Ubyte, 0, 0),
            };
            let _= reg.instance_list.add_instance(name.to_string().clone(), dim_type, mc_support_data, McAddress::new_a2l(addr as u32, 1));
        }
    }



    Ok(())
}
*/

//------------------------------------------------------------------------
//  XCP client

async fn xcp_client(
    tcp: bool,
    dest_addr: std::net::SocketAddr,
    local_addr: std::net::SocketAddr,
    a2l_name: String,
    load_a2l: bool,
    _elf_name: String,
    list_cal: String,
    list_mea: String,
    measurement_list: Vec<String>,
    measurement_time_ms: u64,
    cal_args: Vec<String>,
) -> Result<(), Box<dyn Error>> {
    // Create xcp_client
    let mut xcp_client = XcpClient::new(tcp, dest_addr, local_addr);

    // Connect to the XCP server
    info!("XCP Connect using {}", if tcp { "TCP" } else { "UDP" });
    let daq_decoder = Arc::new(Mutex::new(DaqDecoder::new()));
    xcp_client.connect(Arc::clone(&daq_decoder), ServTextDecoder::new()).await?;
    info!("XCP MAX_CTO = {}", xcp_client.max_cto_size);
    info!("XCP MAX_DTO = {}", xcp_client.max_dto_size);
    info!(
        "XCP RESOURCES = 0x{:02X} {} {} {} {}",
        xcp_client.resources,
        if (xcp_client.resources & 0x01) != 0 { "CAL" } else { "" },
        if (xcp_client.resources & 0x04) != 0 { "DAQ" } else { "" },
        if (xcp_client.resources & 0x10) != 0 { "PGM" } else { "" },
        if (xcp_client.resources & 0x40) != 0 { "STM" } else { "" }
    );
    info!("XCP COMM_MODE_BASIC = 0x{:02X}", xcp_client.comm_mode_basic);
    assert!((xcp_client.comm_mode_basic & 0x07) == 0); // Address granularity != 1 and motorola format not supported
    info!("XCP PROTOCOL_VERSION = 0x{:04X}", xcp_client.protocol_version);
    info!("XCP TRANSPORT_LAYER_VERSION = 0x{:04X}", xcp_client.transport_layer_version);
    info!("XCP DRIVER_VERSION = 0x{:02X}", xcp_client.driver_version);
    info!("XCP MAX_SEGMENTS = {}", xcp_client.max_segments);
    info!("XCP FREEZE_SUPPORTED = {}", xcp_client.freeze_supported);
    info!("XCP MAX_EVENTS = {}", xcp_client.max_events);

    // Get name
    let res = xcp_client.get_id(XCP_IDT_ASCII).await;
    let name = match res {
        Ok((_, Some(id))) => id,
        Err(e) => {
            panic!("GET_ID failed, Error: {}", e);
        }
        _ => {
            panic!("Empty string");
        }
    };
    info!("GET_ID XCP_IDT_ASCII = {}", name);

    // Get EPK
    let res = xcp_client.get_id(XCP_IDT_ASAM_EPK).await;
    let epk = match res {
        Ok((_, Some(id))) => id,
        Err(e) => {
            panic!("GET_ID failed, Error: {}", e);
        }
        _ => {
            panic!("Empty string");
        }
    };
    info!("GET_ID IDT_EPK = {}", epk);

    // Load A2L file from XCP server
    if load_a2l {
        // Get A2L name
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
        info!("GET_ID XCP_IDT_ASAM_NAME = {}", a2l_name);

        // Upload A2L file
        info!("Load A2L file");
        let res = xcp_client.a2l_loader(&a2l_name).await;
        if let Err(e) = res {
            error!("A2L upload failed, Error: {}", e);
            return Err("A2L upload failed".into());
        }
    }
    // Create A2L from segment and event information obtained from the XCP server
    // Add measurement and calibration variables from ELF file if specified
    else {
        // Create an empty A2L registry
        let mut reg = xcp_lite::registry::Registry::new();
        reg.set_app_info(name, "created by xcp_client", 0);
        reg.set_app_version(epk, 0x80000000);
        let protocol = "UDP";
        let addr = Ipv4Addr::new(127, 0, 0, 1);
        let port = 5555;
        reg.set_xcp_params(protocol, addr, port);

        // Get event information
        for i in 0..xcp_client.max_events {
            let name = xcp_client.get_daq_event_info(i).await?;
            info!("Event {}: {}", i, name);
            reg.event_list.add_event(McEvent::new(name, 0, i, 0)).unwrap();
        }

        // Get segment and page information
        for i in 0..xcp_client.max_segments {
            let (addr, length, name) = xcp_client.get_segment_info(i).await?;
            info!("Segment {}: {} addr={:08X} length={} ", i, name, addr, length);
            reg.cal_seg_list.add_cal_seg(name, i as u16, length as u32).unwrap();
        }

        // Read binary file if specified
        if !_elf_name.is_empty() {
            info!("Reading binary file: {}", _elf_name);
            //read_elf(&mut reg, &_elf_name)?;
        }

        let a2l_path = std::path::Path::new(&a2l_name).with_extension("a2l");
        reg.write_a2l(&a2l_path, true).unwrap();
        xcp_client.registry = Some(reg);
    }

    // Print all known calibration objects and get their current value
    if !list_cal.is_empty() {
        println!();
        let cal_objects = xcp_client.find_characteristics(list_cal.as_str());
        println!("Calibration variables:");
        if !cal_objects.is_empty() {
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
        } else {
            println!(" None");
        }
    }

    // Set calibration variable
    if !cal_args.is_empty() {
        if cal_args.len() != 2 {
            return Err("Calibration command requires exactly 2 arguments: variable name and value".into());
        }

        let var_name = &cal_args[0];
        let value_str = &cal_args[1];

        // Parse the value as a double
        let value: f64 = value_str.parse().map_err(|_| format!("Failed to parse '{}' as a double value", value_str))?;

        println!();
        println!("Setting calibration variable '{}' to {}", var_name, value);

        // Create calibration object
        let handle = xcp_client
            .create_calibration_object(var_name)
            .await
            .map_err(|e| format!("Failed to create calibration object for '{}': {}", var_name, e))?;

        // Set the value using f64 (most calibration tools can handle type conversion)
        xcp_client
            .set_value_f64(handle, value)
            .await
            .map_err(|e| format!("Failed to set value for '{}': {}", var_name, e))?;

        println!("Successfully set '{}' = {}", var_name, value);
        println!();
    }

    // Print all known measurement objects
    if !list_mea.is_empty() {
        println!();
        let mea_objects = xcp_client.find_measurements(&list_mea);
        println!("Measurement variables:");
        if !mea_objects.is_empty() {
            for name in &mea_objects {
                if let Some(h) = xcp_client.create_measurement_object(name) {
                    let o = xcp_client.get_measurement_object(h);
                    println!(" {} {} {}", o.get_name(), o.get_a2l_addr(), o.get_a2l_type());
                }
            }
            println!();
        } else {
            println!(" None");
        }
    }

    // Measurement
    if !measurement_list.is_empty() {
        // Create list of measurement variable names
        let list = if measurement_list.len() == 1 {
            // Regular expression
            xcp_client.find_measurements(measurement_list[0].as_str())
        } else {
            // Just a list of names given on the command line
            measurement_list
        };
        if list.is_empty() {
            warn!("No measurement variables found");
        }
        // Start measurement
        else {
            // Create measurement objects for all names in the list
            // Multi dimensional objects not supported yet
            info!("Measurement list:");
            for name in &list {
                if let Some(o) = xcp_client.create_measurement_object(name) {
                    info!(r#"  {}: {}"#, o.0, name);
                }
            }

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
    }

    // Disconnect
    xcp_client.disconnect().await?;

    Ok(())
}

//------------------------------------------------------------------------
// Main function

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    info!("xcp_client");

    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    let log_level = args.log_level.to_log_level_filter();
    env_logger::Builder::new()
        .target(env_logger::Target::Stdout)
        .filter_level(log_level)
        .format_timestamp(None)
        .format_module_path(false)
        .format_target(false)
        .init();

    // Parse IP addresses
    let dest_addr: std::net::SocketAddr = args.dest_addr.parse().map_err(|e| format!("{}", e))?;
    let local_addr: std::net::SocketAddr = args.bind_addr.parse().map_err(|e| format!("{}", e))?;
    info!("XCP server dest addr: {}", dest_addr);
    info!("XCP client local bind addr: {}", local_addr);

    // Run the test executor if --test is specified
    if args.test {
        test_executor(dest_addr, local_addr, TEST_CAL, TEST_DAQ, TEST_DURATION_MS).await
    }
    // Run the XCP client
    else {
        let res = xcp_client(
            args.tcp,
            dest_addr,
            local_addr,
            args.a2l,
            args.load_a2l,
            args.elf,
            args.list_cal,
            args.list_mea,
            args.mea,
            args.time_ms,
            args.cal,
        )
        .await;
        if let Err(e) = res {
            error!("XCP client failed, Error: {}", e);
        }
    }

    Ok(())
}
