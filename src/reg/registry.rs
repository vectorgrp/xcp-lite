//-----------------------------------------------------------------------------
// Module registry
// Registry for calibration segments, parameters and measurement signals

mod a2l_writer;

use a2l_writer::A2lWriter;

use crate::xcp::XcpEvent;

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
            _ => 0.0,
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
            RegistryDataType::Unknown => "UNKNOWN",
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
            RegistryDataType::Unknown => "UNKNOWN",
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
            _ => 0,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Transport layer parameters
// For A2l XCP IF_DATA

#[derive(Clone, Copy, Builder, Debug)]
struct RegistryXcpTransportLayer {
    protocol_name: &'static str,
    ip: [u8; 4],
    port: u16,
}

impl Default for RegistryXcpTransportLayer {
    fn default() -> Self {
        RegistryXcpTransportLayer {
            protocol_name: "UDP",
            ip: [127, 0, 0, 1],
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
        RegistryCalSeg {
            name,
            addr,
            addr_ext,
            size,
        }
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
        RegistryEpk {
            epk: None,
            epk_addr: 0,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Measurement signals

#[derive(Builder, Clone, Debug)]
pub struct RegistryMeasurement {
    name: String,
    datatype: RegistryDataType, // Basic types Ubyte, SByte, AUint64, Float64Ieee, ...  or Blob
    x_dim: u16, // 1 = basic type (A2L MEASUREMENT), >1 = array[dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x (max u16))
    y_dim: u16, // 1 = basic type (A2L MEASUREMENT), >1 = array[x_dim,y_dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x,y (max u16))
    event: XcpEvent,
    addr_offset: i16, // Address offset (signed!) relative to event memory context (XCP_ADDR_EXT_DYN)
    addr:  u64,
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
            annotation
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn datatype(&self) -> RegistryDataType {
        self.datatype
    }
    pub fn dim(&self) -> u16 {
        self.x_dim * self.y_dim
    }
    pub fn x_dim(&self) -> u16 {
        self.x_dim
    }
    pub fn y_dim(&self) -> u16 {
        self.y_dim
    }

    pub fn event(&self) -> XcpEvent {
        self.event
    }

    pub fn addr_offset(&self) -> i16 {
        self.addr_offset
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn factor(&self) -> f64 {
        self.factor
    }

    pub fn offset(&self) -> f64 {
        self.offset
    }

    pub fn comment(&self) -> &str {
        self.comment
    }

    pub fn unit(&self) -> &str {
        self.unit
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

    pub fn iter(&self) -> std::slice::Iter<RegistryMeasurement> {
        self.0.iter()
    }

    fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
    }
}

//-------------------------------------------------------------------------------------------------
// Calibration parameters

#[derive(Builder, Clone, Debug)]
pub struct RegistryCharacteristic {
    calseg_name: &'static str,
    name: String,
    datatype: &'static str,
    comment: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
    x_dim: usize,
    y_dim: usize,
    offset: u16,
    extension: u8, //TODO: Discuss hardcoding extension vs Xcp::get_calseg_ext_addr
}

#[allow(clippy::too_many_arguments)]
impl RegistryCharacteristic {
    pub fn new(
        calseg_name: &'static str,
        name: String,
        datatype: &'static str,
        comment: &'static str,
        min: f64,
        max: f64,
        unit: &'static str,
        x_dim: usize,
        y_dim: usize,
        offset: u16,
        extension: u8,
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
            offset,
            extension,
        }
    }

    pub fn calseg_name(&self) -> &'static str {
        self.calseg_name
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn datatype(&self) -> &str {
        self.datatype
    }

    pub fn comment(&self) -> &str {
        self.comment
    }

    pub fn min(&self) -> f64 {
        self.min
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn unit(&self) -> &str {
        self.unit
    }

    pub fn x_dim(&self) -> usize {
        self.x_dim
    }

    pub fn y_dim(&self) -> usize {
        self.y_dim
    }

    pub fn characteristic_type(&self) -> &'static str {
        if self.x_dim > 1 && self.y_dim > 1 {
            "MAP"
        } else if self.x_dim > 1 || self.y_dim > 1 {
            "CURVE"
        } else {
            "VALUE"
        }
    }

    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub fn extension(&self) -> u8 {
        self.extension
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
            name: None,
            tl_params: None,
            mod_par: RegistryEpk::new(),
            cal_seg_list: RegistryCalSegList::new(),
            characteristic_list: RegistryCharacteristicList::new(),
            event_list: RegistryEventList::new(),
            measurement_list: RegistryMeasurementList::new(),
        }
    }

    /// Clear
    pub fn clear(&mut self) {
        debug!("Clear and close registry");
        self.name = None;
        self.tl_params = None;
        self.mod_par = RegistryEpk::new();
        self.cal_seg_list = RegistryCalSegList::new();
        self.characteristic_list = RegistryCharacteristicList::new();
        self.event_list = RegistryEventList::new();
        self.measurement_list = RegistryMeasurementList::new();
    }

    /// Set name
    pub fn set_name(&mut self, name: &'static str) {
        debug!("set_name: {}", name);
        self.name = Some(name);
    }

    // Get name
    pub fn get_name(&self) -> Option<&'static str> {
        self.name
    }

    // Set EPK
    pub fn set_epk(&mut self, epk: &'static str, epk_addr: u32) {
        debug!("set_epk: {} 0x{:08X}", epk, epk_addr);
        assert!(self.name.is_some(), "Registry is closed");

        self.mod_par.epk = Some(epk);
        self.mod_par.epk_addr = epk_addr;
    }

    // Get EPK
    pub fn get_epk(&mut self) -> Option<&'static str> {
        self.mod_par.epk
    }

    // Set transport layer parameters
    pub fn set_tl_params(&mut self, protocol_name: &'static str, ip: [u8; 4], port: u16) {
        debug!("set_tl_params: {} {:?} {}", protocol_name, ip, port);
        assert!(self.name.is_some(), "Registry is closed");

        self.tl_params = Some(RegistryXcpTransportLayer {
            protocol_name,
            ip,
            port,
        });
    }

    // Add an event
    pub fn add_event(&mut self, event: XcpEvent) {
        debug!(
            "add_event: num={}, index={}",
            event.get_num(),
            event.get_index()
        );
        assert!(self.name.is_some(), "Registry is closed");

        self.event_list.push(event);
    }

    // Add a calibration segment
    pub fn add_cal_seg(&mut self, name: &'static str, addr: u32, addr_ext: u8, size: u32) {
        debug!(
            "add_cal_seg: {} {}:0x{:08X}-{} ",
            name, addr_ext, addr, size
        );
        assert!(self.name.is_some(), "Registry is closed");

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

        self.cal_seg_list
            .push(RegistryCalSeg::new(name, addr, addr_ext, size));
    }

    /// Add an instance of a measurement signal associated to a measurement events
    /// The event index (for multi instance events) is appended to the name
    /// # panics
    ///   If a measurement with the same name already exists
    ///   If the registry is closed
    pub fn add_measurement(&mut self, mut m: RegistryMeasurement) {
        debug!(
            "add_measurement: {} type={:?}[{},{}] event={}+({})",
            m.name,
            m.datatype,
            m.x_dim,
            m.y_dim,
            m.event.get_num(),
            m.addr_offset
        );

        // Panic if registry is closed
        assert!(self.name.is_some(), "Registry is closed");

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
        debug!(
            "add_characteristic: {}/{} type={:?} offset={}",
            c.calseg_name(),
            c.name(),
            c.datatype(),
            c.offset()
        );

        // Panic if registry is closed
        assert!(self.name.is_some(), "Registry is closed");

        // Panic if duplicate
        for c1 in self.characteristic_list.iter() {
            if c.name == c1.name() {
                panic!("Duplicate characteristic: {}", c.name);
            }
        }

        // Check dimensions
        assert!(c.x_dim > 0);
        assert!(c.y_dim > 0);

        self.characteristic_list.push(c);
    }

    pub fn find_characteristic(&self, name: &str) -> Option<&RegistryCharacteristic> {
        self.characteristic_list.iter().find(|c| c.name() == name)
    }

    /// Generate A2L file from registry
    /// Returns true, if file is rewritten due to changes
    pub fn write(&mut self) -> Result<bool, &'static str> {
        // Error if registry is closed
        if self.name.is_none() {
            return Err("Registry is closed");
        }

        // Sort measurement and calibration lists to get deterministic order
        // Event and CalSeg lists stay in the order they were added
        self.measurement_list.sort();
        self.characteristic_list.sort();

        // Write to A2L file
        let writer = A2lWriter::new();
        writer.write_a2l(self)
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
        r.set_tl_params("UDP", [127, 0, 0, 1], 5555);
        r.add_cal_seg("test_memory_segment_1", 0x80010000, 0, 4);
        r.add_cal_seg("test_memory_segment_2", 0x80020000, 0, 4);

        let event = xcp.create_event("test_event", false);
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
            Some("annotation".to_string())
        ));

        std::fs::remove_file("test.a2h").ok();
        let res = r.write();
        let updated = res.expect("A2L write write failed");
        assert!(updated);
        let res = r.write(); // Write again and it should not be written
        let updated = res.expect("A2L write write failed");
        assert!(!updated);

        std::fs::remove_file("test.a2h").ok();
        std::fs::remove_file("test.a2l").ok();
    }
}
