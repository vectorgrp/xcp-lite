//-----------------------------------------------------------------------------
// Module registry
// Registry for calibration segments, parameters and measurement signals

#![allow(dead_code)]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use core::panic;
use std::{borrow::Cow, net::Ipv4Addr};

use crate::xcp;
use xcp::XcpEvent;

mod a2l_writer;
use a2l_writer::A2lWriter;

//----------------------------------------------------------------------------------------------
// Registry error

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("registry error: duplicate symbol `{0}` ")]
    Duplicate(Cow<'static, str>),

    #[error("registry error: `{0}` not found")]
    NotFound(&'static str),

    #[error("unknown error")]
    Unknown,
}

//-------------------------------------------------------------------------------------------------
// Datatype

/// Basic registry data type enum (with ASAM naming convention)
/// Used by the register macros
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
    /// Get minimum value for data type
    /// Used by the register macros
    pub fn get_min(self) -> f64 {
        match self {
            RegistryDataType::Sbyte => i8::MIN as f64,
            RegistryDataType::Sword => i16::MIN as f64,
            RegistryDataType::Slong => i32::MIN as f64,
            RegistryDataType::AInt64 | RegistryDataType::Float32Ieee | RegistryDataType::Float64Ieee => -1E12,
            _ => 0.0,
        }
    }

    fn get_min_str(self) -> &'static str {
        match self {
            RegistryDataType::Sbyte => "-128",
            RegistryDataType::Sword => "-32768",
            RegistryDataType::Slong => "-2147483648",
            RegistryDataType::AInt64 | RegistryDataType::Float32Ieee | RegistryDataType::Float64Ieee => "-1E15",
            _ => "0",
        }
    }

    /// Get maximum value for data type
    /// Used by the register macros
    pub fn get_max(self) -> f64 {
        match self {
            RegistryDataType::Ubyte => u8::MAX as f64,
            RegistryDataType::Sbyte => i8::MAX as f64,
            RegistryDataType::Uword => u16::MAX as f64,
            RegistryDataType::Sword => i16::MAX as f64,
            RegistryDataType::Ulong => u32::MAX as f64,
            RegistryDataType::Slong => i32::MAX as f64,
            RegistryDataType::AUint64 | RegistryDataType::AInt64 | RegistryDataType::Float32Ieee | RegistryDataType::Float64Ieee => 1E12,
            RegistryDataType::Blob => 0.0,
            RegistryDataType::Unknown => panic!("get_max: Unsupported data type"),
        }
    }

    pub fn get_max_str(self) -> &'static str {
        match self {
            RegistryDataType::Ubyte => "255",
            RegistryDataType::Sbyte => "127",
            RegistryDataType::Uword => "65535",
            RegistryDataType::Sword => "32767",
            RegistryDataType::Ulong => "4294967295",
            RegistryDataType::Slong => "2147483647",
            RegistryDataType::AUint64 | RegistryDataType::AInt64 | RegistryDataType::Float32Ieee | RegistryDataType::Float64Ieee => "1E15",
            RegistryDataType::Blob => "0",
            RegistryDataType::Unknown => panic!("get_max: Unsupported data type"),
        }
    }

    // Get data type as str
    // Used by A2L writer
    fn get_type_str(self) -> &'static str {
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
            RegistryDataType::Unknown => panic!("get_type_str: Unsupported data type"),
        }
    }

    // Get data type as str for A2L deposit
    // Used by A2L writer
    fn get_deposit_str(self) -> &'static str {
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
            RegistryDataType::Unknown => panic!("get_deposit_str: Unsupported data type"),
        }
    }

    /// Get data type size
    /// Used by the register macros
    pub fn get_size(self) -> usize {
        match self {
            RegistryDataType::Ubyte | RegistryDataType::Sbyte => 1,
            RegistryDataType::Uword | RegistryDataType::Sword => 2,
            RegistryDataType::Ulong | RegistryDataType::Slong | RegistryDataType::Float32Ieee => 4,
            RegistryDataType::AUint64 | RegistryDataType::AInt64 | RegistryDataType::Float64Ieee => 8,
            RegistryDataType::Blob => 0,
            RegistryDataType::Unknown => panic!("get_size: Unsupported data type"),
        }
    }

    /// Convert from Rust basic type as str
    /// Used by the register macros
    pub fn from_rust_basic_type(s: &str) -> RegistryDataType {
        match s {
            "bool" | "u8" => RegistryDataType::Ubyte,
            "i8" => RegistryDataType::Sbyte,
            "u16" => RegistryDataType::Uword,
            "i16" => RegistryDataType::Sword,
            "u32" => RegistryDataType::Ulong,
            "i32" => RegistryDataType::Slong,
            "u64" | "usize" => RegistryDataType::AUint64, // @@@@ Check if usize is correct
            "i64" | "isize" => RegistryDataType::AInt64,  // @@@@ Check if isize is correct
            "f32" => RegistryDataType::Float32Ieee,
            "f64" => RegistryDataType::Float64Ieee,
            _ => RegistryDataType::Unknown,
        }
    }

    /// Convert from Rust type as str
    /// Used by the register macros
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

//-------------------------------------------------------------------------------------------------
// Get RegistryDataType from rust variables

/// Get RegDataType for a Rust basic type  
/// Glue used by the register_xxx macros
pub trait RegistryDataTypeTrait {
    /// Get RegDataType for a Rust basic type
    fn get_type(&self) -> RegistryDataType;
}

impl<T> RegistryDataTypeTrait for std::num::Wrapping<T>
where
    T: RegistryDataTypeTrait,
{
    fn get_type(&self) -> RegistryDataType {
        self.0.get_type()
    }
}
impl<T> RegistryDataTypeTrait for Option<T>
where
    T: RegistryDataTypeTrait + std::default::Default,
{
    fn get_type(&self) -> RegistryDataType {
        let x: T = T::default();
        x.get_type()
    }
}
impl RegistryDataTypeTrait for bool {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ubyte
    }
}
impl RegistryDataTypeTrait for i8 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Sbyte
    }
}
impl RegistryDataTypeTrait for i16 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Sword
    }
}
impl RegistryDataTypeTrait for i32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Slong
    }
}
impl RegistryDataTypeTrait for i64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AInt64
    }
}
impl RegistryDataTypeTrait for isize {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AInt64
    }
}
impl RegistryDataTypeTrait for u8 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ubyte
    }
}
impl RegistryDataTypeTrait for u16 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Uword
    }
}
impl RegistryDataTypeTrait for u32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Ulong
    }
}
impl RegistryDataTypeTrait for u64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AUint64
    }
}
impl RegistryDataTypeTrait for usize {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::AUint64
    }
}
impl RegistryDataTypeTrait for f32 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Float32Ieee
    }
}
impl RegistryDataTypeTrait for f64 {
    fn get_type(&self) -> RegistryDataType {
        RegistryDataType::Float64Ieee
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

#[derive(Debug, Copy, Clone)]
struct RegistryEvent {
    name: &'static str,
    xcp_event: XcpEvent,
}

#[derive(Debug)]
struct RegistryEventList(Vec<RegistryEvent>);

impl RegistryEventList {
    fn new() -> Self {
        RegistryEventList(Vec::new())
    }
    fn push(&mut self, event: RegistryEvent) {
        self.0.push(event);
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn iter(&self) -> std::slice::Iter<RegistryEvent> {
        self.0.iter()
    }
    fn get_name(&self, xcp_event: XcpEvent) -> Option<&'static str> {
        for event in &self.0 {
            if event.xcp_event == xcp_event {
                return Some(event.name);
            }
        }
        None
    }
}

//-------------------------------------------------------------------------------------------------
// Calibration segments

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
struct RegistryCalSeg {
    name: &'static str,
    index: u16,
    addr: u32,
    addr_ext: u8,
    size: u32,
}

impl RegistryCalSeg {
    fn new(name: &'static str, index: u16, addr: u32, addr_ext: u8, size: u32) -> RegistryCalSeg {
        RegistryCalSeg {
            name,
            index,
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
        RegistryEpk { epk: None, epk_addr: 0 }
    }
}

//-------------------------------------------------------------------------------------------------
// Measurement signals

/// Measurement signal
/// Used by the register macros
#[derive(Clone, Debug)]
pub struct RegistryMeasurement {
    name: Cow<'static, str>,
    // Type
    datatype: RegistryDataType, // Basic types Ubyte, SByte, AUint64, Float64Ieee, ...  or Blob
    x_dim: u16,                 // 1 = basic type (A2L MEASUREMENT), >1 = array[dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x (max u16))
    y_dim: u16,                 // 1 = basic type (A2L MEASUREMENT), >1 = array[x_dim,y_dim] of basic type (A2L MEASUREMENT with MATRIX_DIM x,y (max u16))
    annotation: Option<String>, // For serialized data of variable size (RegistryDataType::Blob)
    // Addressing
    xcp_event: XcpEvent, // Raw XCP event associated with the measurement signal
    addr_offset: i16,    // If addr==0, signed offset relative to event memory context (XCP_ADDR_EXT_DYN)
    addr: u64,           // Pointer (*mut u8 as u64 to be send) to the measurement signal in memory (XCP_ADDR_EXT_ABS)
    // Metadata
    factor: f64,
    offset: f64,
    comment: &'static str,
    unit: &'static str,
}

impl RegistryMeasurement {
    /// Create a new measurement signal
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: &'static str,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        xcp_event: XcpEvent,
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
            name: name.into(),
            datatype,
            x_dim,
            y_dim,
            xcp_event,
            addr_offset: event_offset,
            addr,
            factor,
            offset,
            comment,
            unit,
            annotation,
        }
    }
}

#[derive(Debug)]
struct RegistryMeasurementList(Vec<RegistryMeasurement>);

impl RegistryMeasurementList {
    fn new() -> Self {
        RegistryMeasurementList(Vec::new())
    }

    fn push(&mut self, m: RegistryMeasurement) {
        self.0.push(m);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    fn iter(&self) -> std::slice::Iter<RegistryMeasurement> {
        self.0.iter()
    }

    fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

//-------------------------------------------------------------------------------------------------
// Calibration parameters

/// Calibration parameter
/// Used by the register macros
#[derive(Clone, Debug)]
pub struct RegistryCharacteristic {
    name: Cow<'static, str>,
    // Type
    datatype: RegistryDataType,
    x_dim: usize,
    y_dim: usize,

    // Addressing
    calseg_name: Option<&'static str>, // Name of the calibration segment, if none absolute addressing
    addr_offset: u64,                  // Offset relative to calibration segment (XCP_ADDR_EXT_APP) or absolute address (XCP_ADDR_EXT_ABS) if calseg_name is None
    event: Option<XcpEvent>,           // The event associated with the calibration parameter to enable event triggered measurement

    // Metadata
    comment: &'static str,
    min: f64,
    max: f64,
    unit: &'static str,
}

#[allow(clippy::too_many_arguments)]
impl RegistryCharacteristic {
    /// Create a new calibration parameter
    pub fn new<T: std::convert::Into<Cow<'static, str>>>(
        calseg_name: Option<&'static str>,
        name: T,
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
            name: name.into(),
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

    /// Set the event associated with the calibration parameter
    /// Used by the register macros
    pub fn set_event(&mut self, event: XcpEvent) {
        self.event = Some(event);
    }

    /// Get the A2L object type of the calibration parameter
    fn get_type_str(&self) -> &'static str {
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
struct RegistryCharacteristicList(Vec<RegistryCharacteristic>);

impl RegistryCharacteristicList {
    pub fn new() -> Self {
        RegistryCharacteristicList(Vec::new())
    }

    pub fn push(&mut self, characteristic: RegistryCharacteristic) {
        self.0.push(characteristic);
    }

    pub fn sort(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
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
    pub fn add_event(&mut self, name: &'static str, xcp_event: XcpEvent) {
        debug!("Registry add_event: channel={}, index={}", xcp_event.get_channel(), xcp_event.get_index());
        assert!(!self.is_frozen(), "Registry is closed");

        self.event_list.push(RegistryEvent { name, xcp_event });
    }

    // Add a calibration segment
    pub fn add_cal_seg(&mut self, name: &'static str, index: u16, size: u32) {
        assert!(!self.is_frozen(), "Registry is closed");

        // Length of calseg should be %4 to avoid problems with CANape and checksum calculations
        // Address should also be %4
        if size % 4 != 0 {
            warn!("Calibration segment size should be multiple of 4");
        }

        // Check if name already exists and panic
        for s in self.cal_seg_list.iter() {
            assert!(s.name != name, "Duplicate calibration segment: {}", name);
        }

        // Address calculation
        // Address format for calibration segment field is index | 0x8000 in high word, addr_ext is 0
        // (CANape does not support addr_ext in memory segments)
        let (addr_ext, addr) = crate::Xcp::get_calseg_ext_addr_base(index);

        debug!("Registry add_cal_seg: {} {} {}:0x{:08X}-{} ", name, index, addr_ext, addr, size);

        self.cal_seg_list.push(RegistryCalSeg::new(name, index, addr, addr_ext, size));
    }

    // Get calibration segment index by name
    pub fn get_cal_seg_index(&self, name: &str) -> Option<u16> {
        for s in self.cal_seg_list.iter() {
            if s.name == name {
                return Some(s.index);
            }
        }
        None
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
    pub fn add_measurement(&mut self, mut m: RegistryMeasurement) -> Result<(), RegistryError> {
        debug!(
            "Registry add_measurement: {} type={:?}[{},{}] event={}+({})",
            m.name,
            m.datatype,
            m.x_dim,
            m.y_dim,
            m.xcp_event.get_channel(),
            m.addr_offset
        );

        // Panic if registry is closed
        assert!(!self.is_frozen(), "Registry is closed");

        // Append event index to name in case of a multi instance event (index>0)
        if m.xcp_event.get_index() > 0 {
            m.name = std::borrow::Cow::Owned(format!("{}_{}", m.name, m.xcp_event.get_index()));
        }

        // Panic if symbol_name with same name already exists
        for m1 in self.measurement_list.iter() {
            if m1.name == m.name {
                return Err(RegistryError::Duplicate(m.name));
            }
        }

        // Add to list
        self.measurement_list.push(m);
        Ok(())
    }

    // pub fn find_measurement(&self, name: &str) -> Option<&RegistryMeasurement> {
    //     self.measurement_list.iter().find(|m| m.name == name)
    // }

    /// Add a calibration parameter
    /// # panics
    ///   If a measurement with the same name already exists
    ///   If the registry is closed
    pub fn add_characteristic(&mut self, c: RegistryCharacteristic) -> Result<(), RegistryError> {
        debug!("Registry add_characteristic: {:?}.{} type={:?} offset={}", c.calseg_name, c.name, c.datatype, c.addr_offset);

        // Panic if registry is closed
        assert!(!self.is_frozen(), "Registry is closed");

        // Panic if duplicate
        for c1 in self.characteristic_list.iter() {
            if c.name == c1.name {
                return Err(RegistryError::Duplicate(c.name));
            }
        }

        // Check dimensions
        assert!(c.x_dim > 0);
        assert!(c.y_dim > 0);

        self.characteristic_list.push(c);
        Ok(())
    }

    pub fn find_characteristic(&self, name: &str) -> Option<&RegistryCharacteristic> {
        self.characteristic_list.iter().find(|c| c.name == name)
    }

    #[cfg(feature = "a2l_reader")]
    pub fn a2l_load<P: AsRef<std::path::Path>>(&mut self, filename: P) -> Result<a2lfile::A2lFile, String> {
        let filename = filename.as_ref();
        trace!("Load A2L file {}", filename.display());
        let mut logmsgs = Vec::<a2lfile::A2lError>::new();
        let res = a2lfile::load(filename, None, &mut logmsgs, true);
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
        let mut a2l_path = std::path::PathBuf::from(a2l_name);
        a2l_path.set_extension("a2l");
        let a2l_file = std::fs::File::create(&a2l_path)?;
        info!("Write A2L file {}", a2l_path.display());
        let writer: &mut dyn std::io::Write = &mut std::io::LineWriter::new(a2l_file);
        let mut a2l_writer = A2lWriter::new(writer, self);
        a2l_writer.write_a2l(a2l_name, a2l_name)?;

        // stdout
        // {
        //     let mut stdout = std::io::stdout().lock();
        //     let mut a2l_writer = A2lWriter::new(&mut stdout, self);
        //     a2l_writer.write_a2l(a2l_name, a2l_name)?;
        // }

        // Vec - String - Hash
        // {
        //     let mut vec = std::io::Cursor::new(Vec::with_capacity(1024));
        //     let mut a2l_writer = A2lWriter::new(&mut vec, self);
        //     a2l_writer.write_a2l(a2l_name, a2l_name)?;
        //     let s = String::from_utf8(vec.into_inner()).unwrap();
        //     let mut hasher = std::hash::DefaultHasher::new();
        //     std::hash::Hash::hash(&s.as_str(), &mut hasher);
        //     let a2l_hash: u64 = hasher.finish();
        //     info!("Current A2L hash = {}", a2l_hash);
        // }

        // @@@@ Dev
        // Check A2L file
        #[cfg(feature = "a2l_reader")]
        {
            if let Err(e) = self.a2l_load(a2l_path) {
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

    use super::*;
    use crate::xcp;
    use xcp::*;
    use xcp_type_description::prelude::*;

    //-----------------------------------------------------------------------------
    // Test attribute macros

    #[test]
    fn test_attribute_macros() {
        let xcp = xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
        struct CalPage {
            #[type_description(comment = "Comment")]
            #[type_description(unit = "Unit")]
            #[type_description(min = "0")]
            #[type_description(max = "100")]
            a: u32,
            b: u32,
            curve: [f64; 16],  // This will be a CURVE type (1 dimension)
            map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)
        }
        const CAL_PAGE: CalPage = CalPage {
            a: 1,
            b: 2,
            curve: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
            map: [
                [0, 0, 0, 0, 0, 0, 0, 1, 2],
                [0, 0, 0, 0, 0, 0, 0, 2, 3],
                [0, 0, 0, 0, 0, 1, 1, 2, 3],
                [0, 0, 0, 0, 1, 1, 2, 3, 4],
                [0, 0, 1, 1, 2, 3, 4, 5, 7],
                [0, 1, 1, 1, 2, 4, 6, 8, 9],
                [0, 1, 1, 2, 4, 5, 8, 9, 10],
                [0, 1, 1, 3, 5, 8, 9, 10, 10],
            ],
        };

        let calseg = xcp.create_calseg("calseg", &CAL_PAGE);
        calseg.register_fields();
        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().find_characteristic("CalPage.a").unwrap().clone();

        assert_eq!(calseg.get_name(), "calseg");
        assert_eq!(c.comment, "Comment");
        assert_eq!(c.unit, "Unit");
        assert_eq!(c.min, 0.0);
        assert_eq!(c.max, 100.0);
        assert_eq!(c.x_dim, 1);
        assert_eq!(c.y_dim, 1);
        assert_eq!(c.addr_offset, 200);
        assert_eq!(c.datatype, RegistryDataType::Ulong);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().find_characteristic("CalPage.b").unwrap().clone();
        assert_eq!(c.addr_offset, 204);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().find_characteristic("CalPage.curve").unwrap().clone();
        assert_eq!(c.addr_offset, 0);
        assert_eq!(c.x_dim, 16);
        assert_eq!(c.y_dim, 1);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().find_characteristic("CalPage.map").unwrap().clone();
        assert_eq!(c.addr_offset, 128);
        assert_eq!(c.x_dim, 8);
        assert_eq!(c.y_dim, 9);
    }
}
