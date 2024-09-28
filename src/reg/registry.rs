//-----------------------------------------------------------------------------
// Module registry
// Registry for calibration segments, parameters and measurement signals

use core::panic;
use std::net::Ipv4Addr;

mod a2l_writer;
use a2l_writer::A2lWriter;

use crate::xcp;
use xcp::XcpEvent;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

//-------------------------------------------------------------------------------------------------
// Datatypes and datatype properties

// Basic type (ASAM naming convention)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegistryDataType {
    Ubyte,
    Uword,
    Ulong,
    AUint64,
    Sbyte,
    Sword,
    Slong,
    AInt64,
    Float32Ieee,
    Float64Ieee,
    Blob,
    Unknown,
}

impl RegistryDataType {
    pub fn from_rust_basic_type(s: &str) -> RegistryDataType {
        match s {
            "bool" => RegistryDataType::Ubyte,
            "u8" => RegistryDataType::Ubyte,
            "u16" => RegistryDataType::Uword,
            "u32" => RegistryDataType::Ulong,
            "u64" => RegistryDataType::AUint64,
            "usize" => RegistryDataType::AUint64, // @@@@ Check if usize is correct
            "i8" => RegistryDataType::Sbyte,
            "i16" => RegistryDataType::Sword,
            "i32" => RegistryDataType::Slong,
            "i64" => RegistryDataType::AInt64,
            "isize" => RegistryDataType::AInt64, // @@@@ Check if isize is correct
            "f32" => RegistryDataType::Float32Ieee,
            "f64" => RegistryDataType::Float64Ieee,
            _ => RegistryDataType::Unknown,
        }
    }

    pub fn from_rust_type(s: &str) -> RegistryDataType {
        let t = RegistryDataType::from_rust_basic_type(s);
        if t != RegistryDataType::Unknown {
            t
        } else {
            // Trim leading and trailing whitespace and brackets
            let array_type = s.trim_start_matches('[').trim_end_matches(']');

            // Find the first ';' to handle multi-dimensional arrays
            let first_semicolon_index = array_type.find(';').unwrap_or(array_type.len());

            // Extract the substring from the start to the first ';'
            let inner_type = &array_type[..first_semicolon_index].trim();

            // If there are inner brackets, remove them to get the base type
            let base_type = inner_type.trim_start_matches('[').trim_end_matches(']');

            RegistryDataType::from_rust_basic_type(base_type)
        }
    }
}

// Get RegDataType for a Rust basic type
pub trait RegDataTypeHandler {
    fn get_type(&self) -> RegistryDataType;
}

impl RegDataTypeHandler for bool {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ubyte
    }
}
impl RegDataTypeHandler for i8 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Sbyte
    }
}
impl RegDataTypeHandler for i16 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Sword
    }
}
impl RegDataTypeHandler for i32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Slong
    }
}
impl RegDataTypeHandler for i64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AInt64
    }
}
impl RegDataTypeHandler for u8 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ubyte
    }
}
impl RegDataTypeHandler for u16 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Uword
    }
}
impl RegDataTypeHandler for u32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ulong
    }
}
impl RegDataTypeHandler for u64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AUint64
    }
}
impl RegDataTypeHandler for f32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Float32Ieee
    }
}
impl RegDataTypeHandler for f64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Float64Ieee
    }
}

pub trait RegDataTypeProperties {
    fn get_min(&self) -> f64;
    fn get_max(&self) -> f64;
    fn get_size(&self) -> usize;
    fn get_type_str(&self) -> &'static str;
    fn get_deposit_str(&self) -> &'static str;
}

impl RegDataTypeProperties for RegistryDataType {
    fn get_min(&self) -> f64 {
        match self {
            RegistryDataType::Sbyte => -128.0,
            RegistryDataType::Sword => -32768.0,
            RegistryDataType::Slong => -2147483648.0,
            RegistryDataType::AInt64 => -1e12,
            RegistryDataType::Float32Ieee => -1e12,
            RegistryDataType::Float64Ieee => -1e12,
            _ => 0.0,
        }
    }

    fn get_max(&self) -> f64 {
        match self {
            RegistryDataType::Ubyte => 255.0,
            RegistryDataType::Uword => 65535.0,
            RegistryDataType::Ulong => 4294967295.0,
            RegistryDataType::AUint64 => 1e12,
            RegistryDataType::Sbyte => 127.0,
            RegistryDataType::Sword => 32767.0,
            RegistryDataType::Slong => 2147483647.0,
            RegistryDataType::AInt64 => 1e12,
            RegistryDataType::Float32Ieee => 1e12,
            RegistryDataType::Float64Ieee => 1e12,
            RegistryDataType::Blob => 0.0,
            _ => panic!("get_max: Unsupported data type"),
        }
    }
    fn get_type_str(&self) -> &'static str {
        match self {
            RegistryDataType::Ubyte => "UBYTE",
            RegistryDataType::Uword => "UWORD",
            RegistryDataType::Ulong => "ULONG",
            RegistryDataType::AUint64 => "A_UINT64",
            RegistryDataType::Sbyte => "SBYTE",
            RegistryDataType::Sword => "SWORD",
            RegistryDataType::Slong => "SLONG",
            RegistryDataType::AInt64 => "A_INT64",
            RegistryDataType::Float32Ieee => "FLOAT32_IEEE",
            RegistryDataType::Float64Ieee => "FLOAT64_IEEE",
            RegistryDataType::Blob => "BLOB",
            _ => panic!("get_type_str: Unsupported data type"),
        }
    }
    fn get_deposit_str(&self) -> &'static str {
        match self {
            RegistryDataType::Ubyte => "U8",
            RegistryDataType::Uword => "U16",
            RegistryDataType::Ulong => "U32",
            RegistryDataType::AUint64 => "U64",
            RegistryDataType::Sbyte => "S8",
            RegistryDataType::Sword => "S16",
            RegistryDataType::Slong => "S32",
            RegistryDataType::AInt64 => "S64",
            RegistryDataType::Float32Ieee => "F32",
            RegistryDataType::Float64Ieee => "F64",
            RegistryDataType::Blob => "BLOB",
            _ => panic!("get_deposit_str: Unsupported data type"),
        }
    }
    fn get_size(&self) -> usize {
        match self {
            RegistryDataType::Ubyte => 1,
            RegistryDataType::Uword => 2,
            RegistryDataType::Ulong => 4,
            RegistryDataType::AUint64 => 8,
            RegistryDataType::Sbyte => 1,
            RegistryDataType::Sword => 2,
            RegistryDataType::Slong => 4,
            RegistryDataType::AInt64 => 8,
            RegistryDataType::Float32Ieee => 4,
            RegistryDataType::Float64Ieee => 8,
            RegistryDataType::Blob => 0,
            _ => panic!("get_size: Unsupported data type"),
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Transport layer parameters
// For A2l XCP IF_DATA

#[derive(Clone, Copy, Debug)]
struct RegistryXcpTransportLayer {
    protocol_name: &'static str,
    addr: Ipv4Addr,
    port: u16,
}

impl Default for RegistryXcpTransportLayer {
    fn default() -> Self {
        RegistryXcpTransportLayer {
            protocol_name: "UDP",
            addr: Ipv4Addr::new(127, 0, 0, 1),
            port: 5555,
        }
    }
}

//----------------------------------------------------------------------------------------------
// Events
// For A2l XCP IF_DATA

#[derive(Debug)]
struct RegistryEventList(Vec<XcpEvent>);

impl RegistryEventList {
    fn new() -> Self {
        RegistryEventList(Vec::new())
    }
    pub fn push(&mut self, event: XcpEvent) {
        self.0.push(event);
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn iter(&self) -> std::slice::Iter<XcpEvent> {
        self.0.iter()
    }
}

//-------------------------------------------------------------------------------------------------
// Calibration segments

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
struct RegistryCalSeg {
    name: &'static str,
    addr: u32,
    addr_ext: u8,
    size: u32,
}

impl RegistryCalSeg {
    fn new(name: &'static str, addr: u32, addr_ext: u8, size: u32) -> RegistryCalSeg {
        RegistryCalSeg { name, addr, addr_ext, size }
    }
}

#[derive(Debug)]
struct RegistryCalSegList(Vec<RegistryCalSeg>);

impl RegistryCalSegList {
    fn new() -> Self {
        RegistryCalSegList(Vec::new())
    }
    fn push(&mut self, c: RegistryCalSeg) {
        self.0.push(c);
    }

    fn iter(&self) -> std::slice::Iter<RegistryCalSeg> {
        self.0.iter()
    }
}

//-------------------------------------------------------------------------------------------------
// EPK software version id

#[derive(Debug)]
struct RegistryEpk {
    epk: Option<&'static str>,
    epk_addr: u32,
}

impl RegistryEpk {
    fn new() -> RegistryEpk {
        RegistryEpk { epk: None, epk_addr: 0 }
    }
}

//-------------------------------------------------------------------------------------------------
// Measurement signals

#[derive(Clone, Debug)]
pub struct RegistryMeasurement {
    name: String,
    datatype: RegistryDataType, // Basic types Ubyte, SByte, AUint64, Float64Ieee, ...  or Blob
    x_dim: u16,                 // 1 = basic type (A2L MEASUREMENT), >1 = array[dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x (max u16))
    y_dim: u16,                 // 1 = basic type (A2L MEASUREMENT), >1 = array[x_dim,y_dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x,y (max u16))
    event: XcpEvent,
    addr_offset: i16, // Address offset (signed!) relative to event memory context (XCP_ADDR_EXT_DYN)
    addr: u64,
    factor: f64,
    offset: f64,
    comment: &'static str,
    unit: &'static str,
    annotation: Option<String>,
}

impl RegistryMeasurement {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        event: XcpEvent,
        event_offset: i16,
        addr: u64,
        factor: f64,
        offset: f64,
        comment: &'static str,
        unit: &'static str,
        annotation: Option<String>,
    ) -> Self {
        assert!((x_dim as usize * y_dim as usize) * datatype.get_size() <= u16::MAX as usize / 2);
        RegistryMeasurement {
            name,
            datatype,
            x_dim,
            y_dim,
            event,
            addr_offset: event_offset,
            addr,
            factor,
            offset,
            comment,
            unit,
            annotation,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_datatype(&self) -> RegistryDataType {
        self.datatype
    }
    pub fn get_dim(&self) -> (u16, u16) {
        (self.x_dim, self.y_dim)
    }
    pub fn get_event(&self) -> XcpEvent {
        self.event
    }
    pub fn get_addr_offset(&self) -> i16 {
        self.addr_offset
    }
    pub fn get_addr(&self) -> u64 {
        self.addr
    }
    pub fn get_factor(&self) -> f64 {
        self.factor
    }
    pub fn get_offset(&self) -> f64 {
        self.offset
    }
    pub fn get_comment(&self) -> &str {
        self.comment
    }
    pub fn get_unit(&self) -> &str {
        self.unit
    }
    pub fn get_annotation(&self) -> Option<&String> {
        self.annotation.as_ref()
    }
}

#[derive(Debug)]
struct RegistryMeasurementList(Vec<RegistryMeasurement>);

impl RegistryMeasurementList {
    pub fn new() -> Self {
        RegistryMeasurementList(Vec::new())
    }

    pub fn push(&mut self, m: RegistryMeasurement) {
        self.0.push(m);
    }

    // pub fn len(&self) -> usize {
    //     self.0.len()
    // }

    pub fn iter(&self) -> std::slice::Iter<RegistryMeasurement> {
        self.0.iter()
    }

    fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    }
}

//-------------------------------------------------------------------------------------------------
// Calibration parameters

#[derive(Clone, Debug)]
pub struct RegistryCharacteristic {
    calseg_name: Option<&'static str>,
    name: String,
    datatype: RegistryDataType,
    comment: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
    x_dim: usize,
    y_dim: usize,
    addr_offset: u64,
    event: Option<XcpEvent>,
}

#[allow(clippy::too_many_arguments)]
impl RegistryCharacteristic {
    pub fn new(
        calseg_name: Option<&'static str>,
        name: String,
        datatype: RegistryDataType,
        comment: &'static str,
        min: f64,
        max: f64,
        unit: &'static str,
        x_dim: usize,
        y_dim: usize,
        addr_offset: u64,
    ) -> Self {
        RegistryCharacteristic {
            calseg_name,
            name,
            datatype,
            comment,
            min,
            max,
            x_dim,
            y_dim,
            unit,
            addr_offset,
            event: None,
        }
    }

    pub fn get_calseg_name(&self) -> Option<&'static str> {
        self.calseg_name
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_datatype(&self) -> RegistryDataType {
        self.datatype
    }
    pub fn get_comment(&self) -> &str {
        self.comment
    }
    pub fn get_min(&self) -> f64 {
        self.min
    }
    pub fn get_max(&self) -> f64 {
        self.max
    }
    pub fn get_unit(&self) -> &str {
        self.unit
    }
    pub fn get_x_dim(&self) -> usize {
        self.x_dim
    }
    pub fn get_y_dim(&self) -> usize {
        self.y_dim
    }
    pub fn get_addr_offset(&self) -> u64 {
        self.addr_offset
    }

    pub fn get_event(&self) -> Option<XcpEvent> {
        self.event
    }
    pub fn set_event(&mut self, event: XcpEvent) {
        self.event = Some(event);
    }

    pub fn get_type_str(&self) -> &'static str {
        if self.x_dim > 1 && self.y_dim > 1 {
            "MAP"
        } else if self.x_dim > 1 || self.y_dim > 1 {
            "CURVE"
        } else {
            "VALUE"
        }
    }
}

#[derive(Debug)]
pub struct RegistryCharacteristicList(Vec<RegistryCharacteristic>);

impl RegistryCharacteristicList {
    pub fn new() -> Self {
        RegistryCharacteristicList(Vec::new())
    }

    pub fn push(&mut self, characteristic: RegistryCharacteristic) {
        self.0.push(characteristic);
    }

    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    }

    pub fn iter(&self) -> std::slice::Iter<RegistryCharacteristic> {
        self.0.iter()
    }
}

//-------------------------------------------------------------------------------------------------
// Registry

#[derive(Debug)]
pub struct Registry {
    freeze: bool,
    name: Option<&'static str>,
    tl_params: Option<RegistryXcpTransportLayer>,
    mod_par: RegistryEpk,
    cal_seg_list: RegistryCalSegList,
    characteristic_list: RegistryCharacteristicList,
    event_list: RegistryEventList,
    measurement_list: RegistryMeasurementList,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Create a measurement and calibration registry
    pub fn new() -> Registry {
        Registry {
            freeze: false,
            name: None,
            tl_params: None,
            mod_par: RegistryEpk::new(),
            cal_seg_list: RegistryCalSegList::new(),
            characteristic_list: RegistryCharacteristicList::new(),
            event_list: RegistryEventList::new(),
            measurement_list: RegistryMeasurementList::new(),
        }
    }

    /// Clear (for test only)
    pub fn clear(&mut self) {
        debug!("Registry clear()");
        self.freeze = false;
        self.name = None;
        self.tl_params = None;
        self.mod_par = RegistryEpk::new();
        self.cal_seg_list = RegistryCalSegList::new();
        self.characteristic_list = RegistryCharacteristicList::new();
        self.event_list = RegistryEventList::new();
        self.measurement_list = RegistryMeasurementList::new();
    }

    /// Freeze registry
    pub fn freeze(&mut self) {
        debug!("Registry freeze()");
        self.freeze = true;
    }

    /// Get freeze status   
    pub fn is_frozen(&self) -> bool {
        self.freeze
    }

    /// Set name
    pub fn set_name(&mut self, name: &'static str) {
        debug!("Registry set_name({})", name);
        self.name = Some(name);
    }

    // Get name
    pub fn get_name(&self) -> Option<&'static str> {
        self.name
    }

    // Set EPK
    pub fn set_epk(&mut self, epk: &'static str, epk_addr: u32) {
        debug!("Registry set_epk: {} 0x{:08X}", epk, epk_addr);
        self.mod_par.epk = Some(epk);
        self.mod_par.epk_addr = epk_addr;
    }

    // Get EPK
    pub fn get_epk(&mut self) -> Option<&'static str> {
        self.mod_par.epk
    }

    // Set transport layer parameters
    pub fn set_tl_params(&mut self, protocol_name: &'static str, addr: Ipv4Addr, port: u16) {
        debug!("Registry set_tl_params: {} {} {}", protocol_name, addr, port);
        self.tl_params = Some(RegistryXcpTransportLayer { protocol_name, addr, port });
    }

    // Add an event
    pub fn add_event(&mut self, event: XcpEvent) {
        debug!("Registry add_event: channel={}, index={}", event.get_channel(), event.get_index());
        assert!(!self.is_frozen(), "Registry is closed");

        self.event_list.push(event);
    }

    // Add a calibration segment
    pub fn add_cal_seg(&mut self, name: &'static str, addr: u32, addr_ext: u8, size: u32) {
        debug!("Registry add_cal_seg: {} {}:0x{:08X}-{} ", name, addr_ext, addr, size);
        assert!(!self.is_frozen(), "Registry is closed");

        // Length of calseg should be %4 to avoid problems with CANape and checksum calculations
        // Address should also be %4
        if size % 4 != 0 {
            warn!("Calibration segment size should be multiple of 4");
        }
        if addr % 4 != 0 {
            warn!("Calibration segment address should be multiple of 4");
        }

        // Check if name already exists and panic
        for s in self.cal_seg_list.iter() {
            assert!(s.name != name, "Duplicate calibration segment: {}", name);
        }

        self.cal_seg_list.push(RegistryCalSeg::new(name, addr, addr_ext, size));
    }

    pub fn get_measurement_list(&self) -> &Vec<RegistryMeasurement> {
        println!("Registry get_measurement_list, len = {}", self.measurement_list.0.len());
        &self.measurement_list.0
    }

    /// Add an instance of a measurement signal associated to a measurement events
    /// The event index (for multi instance events) is appended to the name
    /// # panics
    ///   If a measurement with the same name already exists
    ///   If the registry is closed
    pub fn add_measurement(&mut self, mut m: RegistryMeasurement) {
        debug!(
            "Registry add_measurement: {} type={:?}[{},{}] event={}+({})",
            m.name,
            m.datatype,
            m.x_dim,
            m.y_dim,
            m.event.get_channel(),
            m.addr_offset
        );

        // Panic if registry is closed
        assert!(!self.is_frozen(), "Registry is closed");

        // Append event index to name in case of a multi instance event (index>0)
        if m.event.get_index() > 0 {
            m.name = format!("{}_{}", m.name, m.event.get_index())
        }

        // Panic if symbol_name with same name already exists
        for m1 in self.measurement_list.iter() {
            if m1.name == m.name {
                panic!("Duplicate measurement: {}", m.name);
            }
        }

        // Add to list
        self.measurement_list.push(m);
    }

    // pub fn find_measurement(&self, name: &str) -> Option<&RegistryMeasurement> {
    //     self.measurement_list.iter().find(|m| m.name == name)
    // }

    /// Add a calibration parameter
    /// # panics
    ///   If a measurement with the same name already exists
    ///   If the registry is closed
    pub fn add_characteristic(&mut self, c: RegistryCharacteristic) {
        debug!("Registry add_characteristic: {:?}.{} type={:?} offset={}", c.calseg_name, c.name, c.datatype, c.addr_offset);

        // Panic if registry is closed
        assert!(!self.is_frozen(), "Registry is closed");

        // Panic if duplicate
        for c1 in self.characteristic_list.iter() {
            if c.name == c1.name {
                panic!("Duplicate characteristic: {}", c.name);
            }
        }

        // Check dimensions
        assert!(c.x_dim > 0);
        assert!(c.y_dim > 0);

        self.characteristic_list.push(c);
    }

    pub fn find_characteristic(&self, name: &str) -> Option<&RegistryCharacteristic> {
        self.characteristic_list.iter().find(|c| c.name == name)
    }

    #[cfg(feature = "a2l_reader")]
    pub fn a2l_load(&mut self, filename: &str) -> Result<a2lfile::A2lFile, String> {
        trace!("Load A2L file {}", filename);
        let input_filename = &std::ffi::OsString::from(filename);
        let mut logmsgs = Vec::<a2lfile::A2lError>::new();
        let res = a2lfile::load(input_filename, None, &mut logmsgs, true);
        for log_msg in logmsgs {
            warn!("A2l Loader: {}", log_msg);
        }
        match res {
            Ok(a2l_file) => {
                // Perform a consistency check
                let mut logmsgs = Vec::<String>::new();
                a2l_file.check(&mut logmsgs);
                for log_msg in logmsgs {
                    warn!("A2l Checker: {}", log_msg);
                }
                Ok(a2l_file)
            }

            Err(e) => Err(format!("a2lfile::load failed: {:?}", e)),
        }
    }

    /// Generate A2L file from registry
    pub fn write_a2l(&mut self) -> Result<(), std::io::Error> {
        // Error if registry is closed
        if self.is_frozen() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Registry is closed"));
        }

        // Sort measurement and calibration lists to get deterministic order
        // Event and CalSeg lists stay in the order the were added
        self.measurement_list.sort();
        self.characteristic_list.sort();

        // Write to A2L file
        let a2l_name = self.name.unwrap();
        let a2l_path = format!("{}.a2l", a2l_name);
        let a2l_file = std::fs::File::create(&a2l_path)?;
        let a2l_file_writer: &mut dyn std::io::Write = &mut std::io::LineWriter::new(a2l_file);
        let mut writer = A2lWriter::new(a2l_file_writer);
        writer.write_a2l(a2l_name, a2l_name, self)?;

        // @@@@ Dev
        // Check A2L file
        #[cfg(feature = "a2l_reader")]
        {
            if let Err(e) = self.a2l_load(&a2l_path) {
                error!("A2l file check error: {}", e);
            } else {
                info!("A2L file check ok");
            }
        }

        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod registry_tests {

    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::xcp;
    use xcp::*;

    //-----------------------------------------------------------------------------
    // Test A2L writer
    #[test]
    fn test_a2l_writer() {
        xcp_test::test_setup(log::LevelFilter::Info);

        let xcp = Xcp::get();

        let a = Arc::new(Mutex::new(Registry::new()));
        let mut r = a.lock().unwrap();

        r.set_name("test");
        r.set_epk("TEST_EPK", 0x80000000);
        r.set_tl_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);
        r.add_cal_seg("test_memory_segment_1", 0x80010000, 0, 4);
        r.add_cal_seg("test_memory_segment_2", 0x80020000, 0, 4);

        let event = xcp.create_event("test_event");
        r.add_measurement(RegistryMeasurement::new(
            "signal1".to_string(),
            RegistryDataType::Float64Ieee,
            1,
            1,
            event,
            0,
            0,
            1.0,
            0.0,
            "unit",
            "comment",
            Some("annotation".to_string()),
        ));

        r.write_a2l().unwrap();

        // Check update optimization
        //std::fs::remove_file("test.a2h").ok();
        // let res = r.write();
        // let updated = res.expect("A2L write write failed");
        // assert!(updated);
        // let res = r.write(); // Write again and it should not be written
        // let updated = res.expect("A2L write write failed");
        // assert!(!updated);
    }
}
