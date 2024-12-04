//--------------------------------------------------------------------------------------------------------------------------------------------------
// Module xcp_client
// Simplified, quick and dirty implementation of an UDP XCP client for integration testing

#![allow(dead_code)] // because of all the unused XCP definitions

//#![allow(unused_imports)]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use byteorder::{LittleEndian, ReadBytesExt};
use bytes::{BufMut, BytesMut};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::{timeout, Duration};

#[allow(unused_imports)]
use crate::a2l::a2l_reader::{
    a2l_find_characteristic, a2l_find_measurement, a2l_get_characteristics, a2l_get_measurements, a2l_load, a2l_printf_info, A2lAddr, A2lLimits, A2lType,
};

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP Parameters

pub const CMD_TIMEOUT: Duration = Duration::from_secs(3);

pub const XCPTL_MAX_SEGMENT_SIZE: usize = 2048 * 2;

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP error type

// XCP command response codes
pub const CRC_CMD_OK: u8 = 0x00;
pub const CRC_CMD_SYNCH: u8 = 0x00;
pub const CRC_CMD_PENDING: u8 = 0x01;
pub const CRC_CMD_IGNORED: u8 = 0x02;
pub const CRC_CMD_BUSY: u8 = 0x10;
pub const CRC_DAQ_ACTIVE: u8 = 0x11;
pub const CRC_PRM_ACTIVE: u8 = 0x12;
pub const CRC_CMD_UNKNOWN: u8 = 0x20;
pub const CRC_CMD_SYNTAX: u8 = 0x21;
pub const CRC_OUT_OF_RANGE: u8 = 0x22;
pub const CRC_WRITE_PROTECTED: u8 = 0x23;
pub const CRC_ACCESS_DENIED: u8 = 0x24;
pub const CRC_ACCESS_LOCKED: u8 = 0x25;
pub const CRC_PAGE_NOT_VALID: u8 = 0x26;
pub const CRC_PAGE_MODE_NOT_VALID: u8 = 0x27;
pub const CRC_SEGMENT_NOT_VALID: u8 = 0x28;
pub const CRC_SEQUENCE: u8 = 0x29;
pub const CRC_DAQ_CONFIG: u8 = 0x2A;
pub const CRC_MEMORY_OVERFLOW: u8 = 0x30;
pub const CRC_GENERIC: u8 = 0x31;
pub const CRC_VERIFY: u8 = 0x32;
pub const CRC_RESOURCE_TEMPORARY_NOT_ACCESSIBLE: u8 = 0x33;
pub const CRC_SUBCMD_UNKNOWN: u8 = 0x34;
pub const CRC_TIMECORR_STATE_CHANGE: u8 = 0x35;

pub const ERROR_CMD_TIMEOUT: u8 = 0xF0;
pub const ERROR_TL_HEADER: u8 = 0xF1;
pub const ERROR_A2L: u8 = 0xF2;
pub const ERROR_LIMIT: u8 = 0xF3;
pub const ERROR_ODT_SIZE: u8 = 0xF4;

#[derive(Default)]
pub struct XcpError {
    code: u8,
    cmd: u8,
}

impl XcpError {
    pub fn new(code: u8, cmd: u8) -> XcpError {
        XcpError { code, cmd }
    }
    pub fn get_error_code(&self) -> u8 {
        self.code
    }
}

impl std::fmt::Display for XcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let cmd: XcpCommand = From::from(self.cmd);
        match self.code {
            ERROR_CMD_TIMEOUT => {
                write!(f, "{cmd:?}: Command response timeout")
            }
            ERROR_TL_HEADER => {
                write!(f, "Transport layer header error")
            }
            ERROR_A2L => {
                write!(f, "A2L file error")
            }
            ERROR_LIMIT => {
                write!(f, "Calibration value limit exceeded")
            }
            ERROR_ODT_SIZE => {
                write!(f, "ODT max size exceeded")
            }
            CRC_CMD_SYNCH => {
                write!(f, "SYNCH")
            }
            CRC_CMD_PENDING => {
                write!(f, "XCP command PENDING")
            }
            CRC_CMD_IGNORED => {
                write!(f, "{cmd:?}: XCP command IGNORED")
            }
            CRC_CMD_BUSY => {
                write!(f, "{cmd:?}: XCP command BUSY")
            }
            CRC_DAQ_ACTIVE => {
                write!(f, "{cmd:?}: XCP DAQ ACTIVE")
            }
            CRC_PRM_ACTIVE => {
                write!(f, "{cmd:?}: XCP PRM ACTIVE")
            }
            CRC_CMD_UNKNOWN => {
                write!(f, "Unknown XCP command: {cmd:?} ")
            }
            CRC_CMD_SYNTAX => {
                write!(f, "{cmd:?}: XCP command SYNTAX")
            }
            CRC_OUT_OF_RANGE => {
                write!(f, "{cmd:?}: Parameter out of range")
            }
            CRC_WRITE_PROTECTED => {
                write!(f, "{cmd:?}: Write protected")
            }
            CRC_ACCESS_DENIED => {
                write!(f, "{cmd:?}: Access denied")
            }
            CRC_ACCESS_LOCKED => {
                write!(f, "{cmd:?}: Access locked")
            }
            CRC_PAGE_NOT_VALID => {
                write!(f, "{cmd:?}: Invalid page")
            }
            CRC_PAGE_MODE_NOT_VALID => {
                write!(f, "{cmd:?}: Invalide page mode")
            }
            CRC_SEGMENT_NOT_VALID => {
                write!(f, "{cmd:?}: Invalid segment")
            }
            CRC_SEQUENCE => {
                write!(f, "{cmd:?}: Wrong sequence")
            }
            CRC_DAQ_CONFIG => {
                write!(f, "{cmd:?}: DAQ configuration error")
            }
            CRC_MEMORY_OVERFLOW => {
                write!(f, "{cmd:?}: Memory overflow")
            }
            CRC_GENERIC => {
                write!(f, "{cmd:?}: XCP generic error")
            }
            CRC_VERIFY => {
                write!(f, "{cmd:?}: Verify failed")
            }
            CRC_RESOURCE_TEMPORARY_NOT_ACCESSIBLE => {
                write!(f, "{cmd:?}: Resource temporary not accessible")
            }
            CRC_SUBCMD_UNKNOWN => {
                write!(f, "{cmd:?}: Unknown sub command")
            }
            CRC_TIMECORR_STATE_CHANGE => {
                write!(f, "{cmd:?}: Time correlation state change")
            }
            _ => {
                write!(f, "{cmd:?}: XCP error code = 0x{:0X}", self.code)
            }
        }
    }
}

impl std::fmt::Debug for XcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "XcpError 0x{:02X} - {}", self.code, self)
    }
}

impl std::error::Error for XcpError {}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP commands

// XCP command codes
pub const CC_CONNECT: u8 = 0xFF;
pub const CC_DISCONNECT: u8 = 0xFE;
pub const CC_SHORT_DOWNLOAD: u8 = 0xED;
pub const CC_SYNC: u8 = 0xFC;
pub const CC_GET_ID: u8 = 0xFA;
pub const CC_UPLOAD: u8 = 0xF5;
pub const CC_SHORT_UPLOAD: u8 = 0xF4;
pub const CC_USER: u8 = 0xF1;
pub const CC_NOP: u8 = 0xC1;
pub const CC_SET_CAL_PAGE: u8 = 0xEB;
pub const CC_GET_CAL_PAGE: u8 = 0xEA;
pub const CC_GET_SEGMENT_INFO: u8 = 0xE8;
pub const CC_GET_PAGE_INFO: u8 = 0xE7;
pub const CC_SET_SEGMENT_MODE: u8 = 0xE6;
pub const CC_GET_SEGMENT_MODE: u8 = 0xE5;
pub const CC_COPY_CAL_PAGE: u8 = 0xE4;
pub const CC_CLEAR_DAQ_LIST: u8 = 0xE3;
pub const CC_SET_DAQ_PTR: u8 = 0xE2;
pub const CC_WRITE_DAQ: u8 = 0xE1;
pub const CC_SET_DAQ_LIST_MODE: u8 = 0xE0;
pub const CC_GET_DAQ_LIST_MODE: u8 = 0xDF;
pub const CC_START_STOP_DAQ_LIST: u8 = 0xDE;
pub const CC_START_STOP_SYNCH: u8 = 0xDD;
pub const CC_GET_DAQ_CLOCK: u8 = 0xDC;
pub const CC_READ_DAQ: u8 = 0xDB;
pub const CC_GET_DAQ_PROCESSOR_INFO: u8 = 0xDA;
pub const CC_GET_DAQ_RESOLUTION_INFO: u8 = 0xD9;
pub const CC_GET_DAQ_LIST_INFO: u8 = 0xD8;
pub const CC_GET_DAQ_EVENT_INFO: u8 = 0xD7;
pub const CC_FREE_DAQ: u8 = 0xD6;
pub const CC_ALLOC_DAQ: u8 = 0xD5;
pub const CC_ALLOC_ODT: u8 = 0xD4;
pub const CC_ALLOC_ODT_ENTRY: u8 = 0xD3;
pub const CC_TIME_CORRELATION_PROPERTIES: u8 = 0xC6;

#[derive(Debug)]
enum XcpCommand {
    Connect = CC_CONNECT as isize,
    Disconnect = CC_DISCONNECT as isize,
    ShortDownload = CC_SHORT_DOWNLOAD as isize,
    Upload = CC_UPLOAD as isize,
    ShortUpload = CC_SHORT_UPLOAD as isize,
    User = CC_USER as isize,
    Sync = CC_SYNC as isize,
    Nop = CC_NOP as isize,
    GetId = CC_GET_ID as isize,
    SetCalPage = CC_SET_CAL_PAGE as isize,
    GetCalPage = CC_GET_CAL_PAGE as isize,
    GetSegmentInfo = CC_GET_SEGMENT_INFO as isize,
    GetPageInfo = CC_GET_PAGE_INFO as isize,
    SetSegmentMode = CC_SET_SEGMENT_MODE as isize,
    GetSegmentMode = CC_GET_SEGMENT_MODE as isize,
    CopyCalPage = CC_COPY_CAL_PAGE as isize,
    ClearDaqList = CC_CLEAR_DAQ_LIST as isize,
    SetDaqPtr = CC_SET_DAQ_PTR as isize,
    WriteDaq = CC_WRITE_DAQ as isize,
    SetDaqListMode = CC_SET_DAQ_LIST_MODE as isize,
    GetDaqListMode = CC_GET_DAQ_LIST_MODE as isize,
    StartStopDaqList = CC_START_STOP_DAQ_LIST as isize,
    StartStopSynch = CC_START_STOP_SYNCH as isize,
    GetDaqClock = CC_GET_DAQ_CLOCK as isize,
    ReadDaq = CC_READ_DAQ as isize,
    GetDaqProcessorInfo = CC_GET_DAQ_PROCESSOR_INFO as isize,
    GetDaqResolutionInfo = CC_GET_DAQ_RESOLUTION_INFO as isize,
    GetDaqListInfo = CC_GET_DAQ_LIST_INFO as isize,
    GetDaqEventInfo = CC_GET_DAQ_EVENT_INFO as isize,
    FreeDaq = CC_FREE_DAQ as isize,
    AllocDaq = CC_ALLOC_DAQ as isize,
    AllocOdt = CC_ALLOC_ODT as isize,
    AllocOdtEntry = CC_ALLOC_ODT_ENTRY as isize,
    TimeCorrelationProperties = CC_TIME_CORRELATION_PROPERTIES as isize,
}

impl From<u8> for XcpCommand {
    fn from(code: u8) -> Self {
        match code {
            CC_CONNECT => XcpCommand::Connect,
            CC_DISCONNECT => XcpCommand::Disconnect,
            CC_SHORT_DOWNLOAD => XcpCommand::ShortDownload,
            CC_UPLOAD => XcpCommand::Upload,
            CC_SHORT_UPLOAD => XcpCommand::ShortUpload,
            CC_USER => XcpCommand::User,
            CC_SYNC => XcpCommand::Sync,
            CC_NOP => XcpCommand::Nop,
            CC_GET_ID => XcpCommand::GetId,
            CC_SET_CAL_PAGE => XcpCommand::SetCalPage,
            CC_GET_CAL_PAGE => XcpCommand::GetCalPage,
            CC_GET_SEGMENT_INFO => XcpCommand::GetSegmentInfo,
            CC_GET_PAGE_INFO => XcpCommand::GetPageInfo,
            CC_SET_SEGMENT_MODE => XcpCommand::SetSegmentMode,
            CC_GET_SEGMENT_MODE => XcpCommand::GetSegmentMode,
            CC_COPY_CAL_PAGE => XcpCommand::CopyCalPage,
            CC_CLEAR_DAQ_LIST => XcpCommand::ClearDaqList,
            CC_SET_DAQ_PTR => XcpCommand::SetDaqPtr,
            CC_WRITE_DAQ => XcpCommand::WriteDaq,
            CC_SET_DAQ_LIST_MODE => XcpCommand::SetDaqListMode,
            CC_GET_DAQ_LIST_MODE => XcpCommand::GetDaqListMode,
            CC_START_STOP_DAQ_LIST => XcpCommand::StartStopDaqList,
            CC_START_STOP_SYNCH => XcpCommand::StartStopSynch,
            CC_GET_DAQ_CLOCK => XcpCommand::GetDaqClock,
            CC_READ_DAQ => XcpCommand::ReadDaq,
            CC_GET_DAQ_PROCESSOR_INFO => XcpCommand::GetDaqProcessorInfo,
            CC_GET_DAQ_RESOLUTION_INFO => XcpCommand::GetDaqResolutionInfo,
            CC_GET_DAQ_LIST_INFO => XcpCommand::GetDaqListInfo,
            CC_GET_DAQ_EVENT_INFO => XcpCommand::GetDaqEventInfo,
            CC_FREE_DAQ => XcpCommand::FreeDaq,
            CC_ALLOC_DAQ => XcpCommand::AllocDaq,
            CC_ALLOC_ODT => XcpCommand::AllocOdt,
            CC_ALLOC_ODT_ENTRY => XcpCommand::AllocOdtEntry,
            CC_TIME_CORRELATION_PROPERTIES => XcpCommand::TimeCorrelationProperties,
            _ => panic!("Unknown command code: 0x{:02X}", code),
        }
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP protocol definitions

// XCP id types
pub const XCP_IDT_ASCII: u8 = 0;
pub const XCP_IDT_ASAM_NAME: u8 = 1;
pub const XCP_IDT_ASAM_PATH: u8 = 2;
pub const XCP_IDT_ASAM_URL: u8 = 3;
pub const XCP_IDT_ASAM_UPLOAD: u8 = 4;
pub const XCP_IDT_ASAM_EPK: u8 = 5;

// XCP get/set calibration page mode
const CAL_PAGE_MODE_ECU: u8 = 0x01;
const CAL_PAGE_MODE_XCP: u8 = 0x02;

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Build XCP commands with transport layer header

pub struct XcpCommandBuilder {
    data: BytesMut,
}

impl XcpCommandBuilder {
    pub fn new(command_code: u8) -> XcpCommandBuilder {
        let mut cmd = XcpCommandBuilder {
            data: BytesMut::with_capacity(12),
        };
        cmd.data.put_u16_le(0);
        cmd.data.put_u16_le(0);
        cmd.data.put_u8(command_code);
        cmd
    }
    pub fn add_u8(&mut self, value: u8) -> &mut Self {
        self.data.put_u8(value);
        self
    }

    pub fn add_u8_slice(&mut self, value: &[u8]) -> &mut Self {
        self.data.put_slice(value);
        self
    }

    pub fn add_u16(&mut self, value: u16) -> &mut Self {
        assert!(self.data.len() & 1 == 0, "add_u16: unaligned");
        self.data.put_u16_le(value);
        self
    }

    pub fn add_u32(&mut self, value: u32) -> &mut Self {
        assert!(self.data.len() & 3 == 0, "add_u32: unaligned");
        self.data.put_u32_le(value);
        self
    }

    pub fn build(&mut self) -> &[u8] {
        let mut len: u16 = self.data.len().try_into().unwrap();
        assert!(len >= 5);
        len -= 4;
        self.data[0] = (len & 0xFFu16) as u8;
        self.data[1] = (len >> 8) as u8;
        self.data.as_ref()
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// CalibrationObject
// Describes a calibration object with name, address, type, limits and caches it actual value

#[derive(Debug, Clone, Copy)]
pub struct XcpCalibrationObjectHandle(usize);

#[derive(Debug)]
pub struct XcpCalibrationObject {
    name: String,
    a2l_addr: A2lAddr,
    get_type: A2lType,
    a2l_limits: A2lLimits,
    value: Vec<u8>,
}

impl XcpCalibrationObject {
    pub fn new(name: &str, a2l_addr: A2lAddr, get_type: A2lType, a2l_limits: A2lLimits) -> XcpCalibrationObject {
        XcpCalibrationObject {
            name: name.to_string(),
            a2l_addr,
            get_type,
            a2l_limits,
            value: Vec::new(),
        }
    }

    pub fn get_type(&self) -> A2lType {
        self.get_type
    }

    pub fn set_value(&mut self, bytes: &[u8]) {
        self.value = bytes.to_vec();
    }

    pub fn get_value(&mut self) -> &[u8] {
        &self.value
    }

    pub fn get_value_u64(&self) -> u64 {
        let mut value = 0u64;
        for i in (0..self.get_type.size).rev() {
            value <<= 8;
            value += self.value[i as usize] as u64;
        }
        value
    }

    pub fn get_value_i64(&self) -> i64 {
        let size: usize = self.get_type.size as usize;
        let mut value = 0;
        if self.value[size - 1] & 0x80 != 0 {
            value = -1;
        }
        for i in (0..size).rev() {
            value <<= 8;
            assert!(value & 0xFF == 0);
            value |= self.value[i] as i64;
        }
        value
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// MeasurementObject
// Describes a measurement object with name, address, type and event

#[derive(Debug, Clone)]
pub struct XcpMeasurementObjectHandle(usize);

#[derive(Debug, Clone)]
pub struct XcpMeasurementObject {
    name: String,
    pub a2l_addr: A2lAddr,
    pub a2l_type: A2lType,
    pub daq: u16,
    pub odt: u8,
    pub offset: u16,
}

impl XcpMeasurementObject {
    pub fn new(name: &str, a2l_addr: A2lAddr, a2l_type: A2lType) -> XcpMeasurementObject {
        XcpMeasurementObject {
            name: name.to_string(),
            a2l_addr,
            a2l_type,
            daq: 0,
            odt: 0,
            offset: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_addr(&self) -> A2lAddr {
        self.a2l_addr
    }
    pub fn get_type(&self) -> A2lType {
        self.a2l_type
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Text decoder trait for XCP SERV_TEXT messages

pub trait XcpTextDecoder {
    /// Handle incomming SERV_TEXT data from XCP server
    fn decode(&self, data: &[u8]) {
        print!("SERV_TEXT: ");
        let mut j = 0;
        while j < data.len() {
            if data[j] == 0 {
                break;
            }
            print!("{}", data[j] as char);
            j += 1;
        }
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// DAQ decoder trait for XCP DAQ messages

/// DAQ information
/// Describes a single ODT entry
#[derive(Debug)]
pub struct OdtEntry {
    pub name: String,
    pub a2l_type: A2lType,
    pub a2l_addr: A2lAddr,
    pub offset: u16, // offset from data start, not including daq header and timestamp
}

pub trait XcpDaqDecoder {
    /// Handle incomming DAQ packet from XCP server
    /// Transport layer header has been stripped
    fn decode(&mut self, lost: u32, data: &[u8]);

    /// Measurement start
    /// Decoding information: ODT entry table and 64 bit start timestamp
    fn start(&mut self, odt_entries: Vec<Vec<OdtEntry>>, timestamp_raw64: u64);

    // Measurement stop
    fn stop(&mut self) {}

    /// Set measurement timestamp resolution in ns per raw timestamp tick and DAQ header size (2 (ODTB/DAQB or 4 (ODTB,_,DAQW))
    fn set_daq_properties(&mut self, timestamp_resolution: u64, daq_header_size: u8);
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Type to control the receive task sent over the receive task control channel

#[derive(Debug, Copy, Clone)]
pub struct XcpTaskControl {
    running: bool,
    connected: bool,
}

impl XcpTaskControl {
    #[allow(clippy::new_without_default)]
    pub fn new() -> XcpTaskControl {
        XcpTaskControl { running: false, connected: false }
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XcpClient

/// XCP client
pub struct XcpClient {
    bind_addr: SocketAddr,
    dest_addr: SocketAddr,
    socket: Option<Arc<UdpSocket>>,
    rx_cmd_resp: Option<mpsc::Receiver<Vec<u8>>>,
    tx_task_control: Option<mpsc::Sender<XcpTaskControl>>,
    task_control: XcpTaskControl,
    daq_decoder: Option<Arc<Mutex<dyn XcpDaqDecoder>>>,
    ctr: u16,
    max_cto_size: u8,
    max_dto_size: u16,
    timestamp_resolution_ns: u64,
    daq_header_size: u8,
    a2l_file: Option<a2lfile::A2lFile>,
    calibration_objects: Vec<XcpCalibrationObject>,
    measurement_objects: Vec<XcpMeasurementObject>,
}

impl XcpClient {
    //------------------------------------------------------------------------
    // new
    //
    #[allow(clippy::type_complexity)]
    pub fn new(dest_addr: SocketAddr, bind_addr: SocketAddr) -> XcpClient {
        XcpClient {
            bind_addr,
            dest_addr,
            socket: None,
            rx_cmd_resp: None,
            tx_task_control: None,
            task_control: XcpTaskControl::new(),
            daq_decoder: None,
            ctr: 0,
            max_cto_size: 0,
            max_dto_size: 0,
            timestamp_resolution_ns: 1,
            daq_header_size: 4,
            a2l_file: None,
            calibration_objects: Vec::new(),
            measurement_objects: Vec::new(),
        }
    }

    //------------------------------------------------------------------------
    // receiver task
    // Handle incomming data from XCP server
    async fn receive_task(
        socket: Arc<UdpSocket>,
        tx_resp: Sender<Vec<u8>>,
        mut rx_daq_decoder: Receiver<XcpTaskControl>,
        decode_serv_text: impl XcpTextDecoder,
        decode_daq: Arc<Mutex<impl XcpDaqDecoder>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut ctr_last: u16 = 0;
        let mut ctr_first: bool = true;
        let mut ctr_lost: u32 = 0;

        let mut buf: [u8; 8000] = [0; 8000];
        let mut task_control: Option<XcpTaskControl> = None;

        loop {
            select! {

                // Handle the data from rx_daq_decoder
                res = rx_daq_decoder.recv() => {
                    match res {
                        Some(c) => {
                            debug!("receive_task: task control status changed: connected={} running={}", c.connected, c.running);

                            // Disconnect
                            if !c.connected { // Handle the data from rx_daq_decoder
                                info!("receive_task: stop, disconnect");
                                return Ok(());
                            }

                            // Start DAQ
                            if c.running {
                                info!("receive_task: start DAQ");
                                ctr_first = true;
                                ctr_last = 0;
                                ctr_lost = 0;

                            }

                            task_control = Some(c);
                        }
                        None => { // The sender has been dropped
                            info!("receive_task: stop, channel closed");
                            return Ok(());
                        }
                    }
                } // rx_daq_decoder.recv

                // Handle the data from socket
                res = socket.recv_from(&mut buf) => {
                    match res {
                        Ok((size, _)) => {
                            // Handle the data from recv_from
                            if size == 0 {
                                warn!("xcp_receive: socket closed");
                                return Ok(());
                            }

                            let mut i: usize = 0;
                            while i < size {
                                // Decode the next transport layer message header in the packet
                                if size < 5 {
                                    return Err(Box::new(XcpError::new(ERROR_TL_HEADER,0)) as Box<dyn Error>);
                                }
                                let len = buf[i] as usize + ((buf[i + 1] as usize) << 8);
                                if len > size - 4 || len == 0 { // Corrupt packet received, not enough data received or no content
                                    return Err(Box::new(XcpError::new(ERROR_TL_HEADER,0)) as Box<dyn Error>);
                                }
                                let ctr = buf[i + 2] as u16 + ((buf[i + 3] as u16) << 8);
                                if ctr_first {
                                    ctr_first = false;
                                } else if ctr != ctr_last.wrapping_add(1) {
                                    ctr_lost += ctr.wrapping_sub(ctr_last) as u32;

                                }
                                ctr_last = ctr;
                                let pid = buf[i + 4];
                                trace!("RX: i = {}, len = {}, pid = {}", i, len, pid,);
                                match pid {
                                    0xFF => {
                                        // Command response
                                        let response = &buf[(i + 4)..(i + 4 + len)];
                                        trace!("xcp_receive: XCP response = {:?}", response);
                                        tx_resp.send(response.to_vec()).await?;
                                    }
                                    0xFE => {
                                        // Command error response
                                        let response = &buf[(i + 4)..(i + 6)];
                                        trace!("xcp_receive: XCP error response = {:?}", response);
                                        tx_resp.send(response.to_vec()).await?;
                                    }
                                    0xFD => {
                                        // Event
                                        let event_code = buf[i + 5];
                                        warn!("xcp_receive: ignored XCP event = 0x{:0X}", event_code);
                                    }
                                    0xFC => {
                                        // Service
                                        let service_code = buf[i + 5];
                                        if service_code == 0x01 {
                                            decode_serv_text.decode(&buf[i + 6..i + len + 4]);
                                        } else {
                                            // Unknown PID
                                            warn!(
                                                "xcp_receive: ignored unknown service request code = 0x{:0X}",
                                                service_code
                                            );
                                        }
                                    }
                                    _ => {
                                        // Check that we got a DAQ control
                                        if let Some(c) = &task_control {

                                            // Handle DAQ data if DAQ running
                                            if c.running {
                                                let mut m = decode_daq.lock(); // @@@@ Unnessesary mutex ?????
                                                m.decode(ctr_lost, &buf[i + 4..i + 4 + len]);
                                                ctr_lost = 0;
                                            } // running
                                        }
                                    }
                                } // match pid
                                i = i + len + 4;
                            } // while message in packet


                        }
                        Err(e) => {
                            // Handle the error from recv_from
                            error!("xcp_receive: socket error {}",e);
                            return Err(Box::new(XcpError::new(ERROR_TL_HEADER,0)) as Box<dyn Error>);
                        }
                    }
                } // socket.recv_from
            }
        } // loop
    }

    //------------------------------------------------------------------------
    // XCP command service
    // Send a XCP command and wait for the response
    async fn send_command(&mut self, cmd_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        //
        // Send command
        let socket = self.socket.as_ref().unwrap();
        socket.send_to(cmd_bytes, self.dest_addr).await?;

        // Wait for response channel with timeout
        let res = timeout(CMD_TIMEOUT, self.rx_cmd_resp.as_mut().unwrap().recv()).await; // rx channel
        match res {
            Ok(res) => {
                match res {
                    Some(data) => {
                        trace!("xcp_command: res = {:?}", data);
                        match data[0] {
                            0xFF => {
                                // XCP positive response
                                Ok(data)
                            }
                            0xFE => {
                                // XCP negative response, return error code with XcpError
                                Err(Box::new(XcpError::new(data[1], cmd_bytes[4])) as Box<dyn Error>)
                            }
                            _ => {
                                panic!("xcp_command: bug in receive_task");
                            }
                        }
                    }
                    None => {
                        // @@@@ Empty response, channel has been closed, return with XcpError Timeout
                        error!("xcp_command: receive_task channel closed");
                        Err(Box::new(XcpError::new(ERROR_CMD_TIMEOUT, 0)) as Box<dyn Error>)
                    }
                }
            }
            Err(_) => {
                // Timeout, return with XcpError
                Err(Box::new(XcpError::new(ERROR_CMD_TIMEOUT, cmd_bytes[4])) as Box<dyn Error>)
            }
        }
    }

    //------------------------------------------------------------------------
    // Connect/disconnect to server, create receive task

    pub async fn connect<D, T>(&mut self, daq_decoder: Arc<Mutex<D>>, text_decoder: T) -> Result<(), Box<dyn Error>>
    where
        T: XcpTextDecoder + Send + 'static,
        D: XcpDaqDecoder + Send + 'static,
    {
        // Create socket
        let socket = UdpSocket::bind(self.bind_addr).await?;
        self.socket = Some(Arc::new(socket));

        // Spawn a rx task to handle incomming data
        // Hand over the DAQ decoder and the text decoder
        // Create channels for command responses and DAQ state control
        {
            let socket = Arc::clone(self.socket.as_ref().unwrap());
            let (tx_resp, rx_resp) = mpsc::channel(1);
            self.rx_cmd_resp = Some(rx_resp); // rx XCP command response channel
            let (tx_daq, rx_daq) = mpsc::channel(3);
            self.tx_task_control = Some(tx_daq); // tx XCP DAQ control channel
            let daq_decoder_clone = Arc::clone(&daq_decoder);

            tokio::spawn(async move {
                let _res = XcpClient::receive_task(socket, tx_resp, rx_daq, text_decoder, daq_decoder_clone).await;
            });
            tokio::time::sleep(Duration::from_millis(100)).await; // wait for the receive task to start
        }

        // Connect
        let data = self.send_command(XcpCommandBuilder::new(CC_CONNECT).add_u8(0).build()).await?;
        assert!(data.len() >= 8);
        let max_cto_size: u8 = data[3];
        let max_dto_size: u16 = data[4] as u16 | (data[5] as u16) << 8;
        info!("XCP client connected, max_cto_size = {}, max_dto_size = {}", max_cto_size, max_dto_size);
        self.max_cto_size = max_cto_size;
        self.max_dto_size = max_dto_size;

        // Notify the rx task
        self.task_control.connected = true; // the task will end, when it gets connected = false over the XcpControl channel
        self.task_control.running = false;
        self.tx_task_control.as_ref().unwrap().send(self.task_control).await.unwrap();

        assert!(self.is_connected());

        // Get DAQ properties
        self.get_daq_processor_info().await?;

        // Initialize DAQ clock
        self.time_correlation_properties().await?; // Set 64 bit response format for GET_DAQ_CLOCK
        self.timestamp_resolution_ns = self.get_daq_resolution_info().await?;

        // Set the DAQ decoder
        daq_decoder.lock().set_daq_properties(self.timestamp_resolution_ns, self.daq_header_size);

        // Keep the the DAQ decoder for measurement start
        self.daq_decoder = Some(daq_decoder);

        Ok(())
    }

    //------------------------------------------------------------------------
    pub async fn disconnect(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_DISCONNECT).add_u8(0).build()).await?;

        self.task_control.connected = false;
        self.task_control.running = false;
        self.tx_task_control.as_ref().unwrap().send(self.task_control).await.unwrap();

        Ok(())
    }

    //------------------------------------------------------------------------
    pub fn is_connected(&mut self) -> bool {
        self.task_control.connected
    }

    //------------------------------------------------------------------------
    // Get server identification
    // @@@@ Impl: other types, only  XCP_IDT_ASAM_UPLOAD supported
    pub async fn get_id(&mut self, id_type: u8) -> Result<(u32, Option<String>), Box<dyn Error>> {
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_ID).add_u8(id_type).build()).await?;

        assert_eq!(data[0], 0xFF);
        assert!(id_type == XCP_IDT_ASAM_UPLOAD || id_type == XCP_IDT_ASAM_NAME); // others not supported yet
        let mode = data[1]; // 0 = data by upload, 1 = data in response

        // Decode size
        let mut size = 0u32;
        for i in (4..8).rev() {
            size = size << 8 | data[i] as u32;
        }
        info!("GET_ID mode={} -> size = {}", id_type, size);

        // Data ready for upload
        if mode == 0 {
            Ok((size, None))
        }
        // Data in response
        else {
            // Decode string
            let name = String::from_utf8(data[8..(size as usize + 8)].to_vec());
            match name {
                Ok(name) => {
                    info!("  -> text = {}", name);
                    Ok((0, Some(name)))
                }
                Err(_) => {
                    error!("GET_ID mode={} -> invalid string {:?}", id_type, data);
                    Err(Box::new(XcpError::new(CRC_CMD_SYNTAX, CC_GET_ID)) as Box<dyn Error>)
                }
            }
        }
    }

    //------------------------------------------------------------------------
    // Execute a XCP command with no other parameters
    pub async fn command(&mut self, command_code: u8) -> Result<Vec<u8>, Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(command_code).build()).await
    }

    //------------------------------------------------------------------------
    // calibration segment and page control

    pub async fn get_ecu_page(&mut self) -> Result<u8, Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_ECU | 0x80;
        let segment = 0;
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_CAL_PAGE).add_u8(mode).add_u8(segment).build()).await?;
        let page = if data[3] != 0 { 1 } else { 0 };
        Ok(page)
    }

    pub async fn get_xcp_page(&mut self) -> Result<u8, Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_XCP | 0x80;
        let segment = 0;
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_CAL_PAGE).add_u8(mode).add_u8(segment).build()).await?;
        let page = if data[3] != 0 { 1 } else { 0 };
        Ok(page)
    }

    pub async fn set_ecu_page(&mut self, page: u8) -> Result<(), Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_ECU | 0x80;
        let segment = 0;
        self.send_command(XcpCommandBuilder::new(CC_SET_CAL_PAGE).add_u8(mode).add_u8(segment).add_u8(page).build())
            .await?;
        Ok(())
    }

    pub async fn set_xcp_page(&mut self, page: u8) -> Result<(), Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_XCP | 0x80;
        let segment = 0;
        self.send_command(XcpCommandBuilder::new(CC_SET_CAL_PAGE).add_u8(mode).add_u8(segment).add_u8(page).build())
            .await?;
        Ok(())
    }

    //------------------------------------------------------------------------
    // XCP memory access services (calibration and polling of measurememt vvalues)

    pub async fn short_download(&mut self, addr: u32, ext: u8, data_bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        let len: u8 = data_bytes.len().try_into().unwrap();
        trace!("short_download addr={}:{:08X},{} data={:?}", ext, addr, len, data_bytes);
        self.send_command(
            XcpCommandBuilder::new(CC_SHORT_DOWNLOAD)
                .add_u8(len)
                .add_u8(0)
                .add_u8(ext)
                .add_u32(addr)
                .add_u8_slice(data_bytes)
                .build(),
        )
        .await?;
        Ok(())
    }
    pub async fn short_upload(&mut self, addr: u32, ext: u8, size: u8) -> Result<Vec<u8>, Box<dyn Error>> {
        let data = self
            .send_command(XcpCommandBuilder::new(CC_SHORT_UPLOAD).add_u8(size).add_u8(0).add_u8(ext).add_u32(addr).build())
            .await?;

        Ok(data)
    }

    pub async fn upload(&mut self, size: u8) -> Result<Vec<u8>, Box<dyn Error>> {
        let data = self.send_command(XcpCommandBuilder::new(CC_UPLOAD).add_u8(size).build()).await?;
        Ok(data)
    }

    pub async fn modify_begin(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_USER).add_u8(1).add_u8(0).add_u8(0).build()).await?;
        Ok(())
    }

    pub async fn modify_end(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_USER).add_u8(2).add_u8(0).add_u8(0).build()).await?;
        Ok(())
    }

    //------------------------------------------------------------------------
    // XCP DAQ services

    /// Get DAQ clock timestamp resolution in ns
    pub async fn get_daq_processor_info(&mut self) -> Result<(), Box<dyn Error>> {
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_DAQ_PROCESSOR_INFO).build()).await?;
        let mut c = Cursor::new(&data[1..]);

        let daq_properties = c.read_u8()?;
        assert!((daq_properties & 0x10) == 0x10, "DAQ timestamps must be available");
        let max_daq = c.read_u16::<LittleEndian>()?;
        let max_event = c.read_u16::<LittleEndian>()?;
        let min_daq = c.read_u8()?;
        let daq_key_byte = c.read_u8()?;
        self.daq_header_size = (daq_key_byte >> 6) + 1;
        assert!(self.daq_header_size == 4 || self.daq_header_size == 2, "DAQ header type must be ODT_FIL_DAQW or ODT_DAQB");

        info!(
            "GET_DAQ_PROPERTIES daq_properties = 0x{:0X}, max_daq = {}, max_event = {}, min_daq = {}, daq_key_byte = 0x{:0X} (header_size={})",
            daq_properties, max_daq, max_event, min_daq, daq_key_byte, self.daq_header_size
        );
        Ok(())
    }

    async fn free_daq(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_FREE_DAQ).build()).await?;
        Ok(())
    }

    async fn alloc_daq(&mut self, count: u16) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_ALLOC_DAQ).add_u8(0).add_u16(count).build()).await?;
        Ok(())
    }

    async fn alloc_odt(&mut self, daq: u16, odt: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_ALLOC_ODT).add_u8(0).add_u16(daq).add_u8(odt).build()).await?;
        Ok(())
    }

    async fn alloc_odt_entries(&mut self, daq: u16, odt: u8, count: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_ALLOC_ODT_ENTRY).add_u8(0).add_u16(daq).add_u8(odt).add_u8(count).build())
            .await?;
        Ok(())
    }

    async fn set_daq_ptr(&mut self, daq: u16, odt: u8, idx: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_SET_DAQ_PTR).add_u8(0).add_u16(daq).add_u8(odt).add_u8(idx).build())
            .await?;
        Ok(())
    }

    async fn write_daq(&mut self, ext: u8, addr: u32, len: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_WRITE_DAQ)
                .add_u8(0) // bit offset
                .add_u8(len)
                .add_u8(ext)
                .add_u32(addr)
                .build(),
        )
        .await?;
        Ok(())
    }

    async fn set_daq_list_mode(&mut self, daq: u16, eventchannel: u16) -> Result<(), Box<dyn Error>> {
        const XCP_DAQ_MODE_TIMESTAMP: u8 = 0x10; // Timestamp always on, no other mode supported by XCPlite
        let mode: u8 = XCP_DAQ_MODE_TIMESTAMP;
        let priority = 0x00; // Always use priority 0, no DAQ list flush for specific events, priorization supported by XCPlite
        self.send_command(
            XcpCommandBuilder::new(CC_SET_DAQ_LIST_MODE)
                .add_u8(mode)
                .add_u16(daq)
                .add_u16(eventchannel)
                .add_u8(1) // prescaler
                .add_u8(priority)
                .build(),
        )
        .await?;
        Ok(())
    }

    // Select DAQ list
    async fn select_daq_list(&mut self, daq: u16) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_START_STOP_DAQ_LIST).add_u8(2).add_u16(daq).build()).await?;
        Ok(())
    }

    // Prepare, start selected, stop all
    async fn prepare_selected_daq_lists(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_START_STOP_SYNCH).add_u8(3 /* prepare selected */).build())
            .await?;
        Ok(())
    }
    async fn start_selected_daq_lists(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_START_STOP_SYNCH).add_u8(1 /* start selected */).build())
            .await?;
        Ok(())
    }
    async fn stop_all_daq_lists(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_START_STOP_SYNCH).add_u8(0).build()).await?;
        Ok(())
    }

    //-------------------------------------------------------------------------------------------------
    // Clock

    // CC_TIME_CORRELATION_PROPERTIES
    async fn time_correlation_properties(&mut self) -> Result<(), Box<dyn Error>> {
        let request: u8 = 2; // set responce format to SERVER_CONFIG_RESPONSE_FMT_ADVANCED
        let properties: u8 = 0;
        let cluster_id: u16 = 0;
        let _data = self
            .send_command(
                XcpCommandBuilder::new(CC_TIME_CORRELATION_PROPERTIES)
                    .add_u8(request)
                    .add_u8(properties)
                    .add_u8(0)
                    .add_u16(cluster_id)
                    .build(),
            )
            .await?;
        info!("TIME_CORRELATION_PROPERIES set response format to SERVER_CONFIG_RESPONSE_FMT_ADVANCED");
        Ok(())
    }

    /// Get DAQ clock timestamp resolution in ns
    pub async fn get_daq_resolution_info(&mut self) -> Result<u64, Box<dyn Error>> {
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_DAQ_RESOLUTION_INFO).build()).await?;
        let mut c = Cursor::new(&data[1..]);

        let granularity_daq = c.read_u8()?;
        let max_size_daq = c.read_u8()?;
        let _granularity_stim = c.read_u8()?;
        let _max_size_stim = c.read_u8()?;
        let timestamp_mode = c.read_u8()?;
        let timestamp_ticks = c.read_u16::<LittleEndian>()?;

        assert!(granularity_daq == 0x01, "support only 1 byte DAQ granularity");
        assert!(timestamp_mode & 0x07 == 0x04, "support only 32 bit DAQ timestamps");
        assert!(timestamp_mode & 0x08 == 0x08, "support only fixed DAQ timestamps");

        // Calculate timestamp resolution in ns per tick
        let mut timestamp_unit = timestamp_mode >> 4; // 1ns=0, 10ns=1, 100ns=2, 1us=3, 10us=4, 100us=5, 1ms=6, 10ms=7, 100ms=8, 1s=9
        let mut timestamp_resolution_ns: u64 = timestamp_ticks as u64;
        while timestamp_unit > 0 {
            timestamp_resolution_ns *= 10;
            timestamp_unit -= 1;
        }
        self.timestamp_resolution_ns = timestamp_resolution_ns;

        info!(
            "GET_DAQ_RESOLUTION_INFO granularity_daq={} max_size_daq={} timestamp_mode={} timestamp_resolution={}ns",
            granularity_daq, max_size_daq, timestamp_mode, timestamp_resolution_ns
        );
        Ok(timestamp_resolution_ns)
    }

    // Get DAQ clock raw value in ticks of timestamp_resolution ns
    async fn get_daq_clock_raw(&mut self) -> Result<u64, Box<dyn Error>> {
        let data = self.send_command(XcpCommandBuilder::new(CC_GET_DAQ_CLOCK).build()).await?;
        let mut c = Cursor::new(&data[2..]);

        // Trigger info and payload format
        // TIME_OF_TS_SAMPLING: (trigger_info >> 3) & 0x03 : 3-reception, 2-transmission, 1-low jitter, 0-during commend processing
        // TRIGGER_INITIATOR:   (trigger_info >> 0) & 0x07 : not relevant for GET_DAQ_CLOCK
        // FMT_XCP_SLV: (payload_fmt >> 0) & 0x03 let payload_fmt = data[3];
        let trigger_info = c.read_u8()?;
        let payload_fmt = c.read_u8()?;

        // Timestamp
        let timestamp64 = if payload_fmt == 1 {
            // 32 bit slave clock
            c.read_u32::<LittleEndian>()? as u64
        } else if payload_fmt == 2 {
            // 64 bit slave clock
            c.read_u64::<LittleEndian>()?
        } else {
            return Err(Box::new(XcpError::new(CRC_OUT_OF_RANGE, CC_GET_DAQ_CLOCK)) as Box<dyn Error>);
        };

        trace!("GET_DAQ_CLOCK trigger_info=0x{:2X}, payload_fmt=0x{:2X} time={}", trigger_info, payload_fmt, timestamp64);
        Ok(timestamp64)
    }

    /// Get DAQ clock in ns
    pub async fn get_daq_clock(&mut self) -> Result<u64, Box<dyn Error>> {
        let timestamp64 = self.get_daq_clock_raw().await?;
        let timestamp_ns = timestamp64 * self.timestamp_resolution_ns;
        Ok(timestamp_ns)
    }

    //-------------------------------------------------------------------------------------------------
    // A2L upload and load

    /// Upload A2l
    pub async fn upload_a2l(&mut self, print_info: bool) -> Result<(), Box<dyn Error>> {
        self.a2l_loader::<&str>(None, print_info).await
    }

    /// Load A2L
    pub async fn read_a2l<P: AsRef<Path>>(&mut self, filename: P, print_info: bool) -> Result<(), Box<dyn Error>> {
        self.a2l_loader(Some(filename), print_info).await
    }

    // Get the A2L via XCP or from file and read it
    pub async fn a2l_loader<P: AsRef<Path>>(&mut self, filename: Option<P>, print_info: bool) -> Result<(), Box<dyn Error>> {
        let a2l_filename = filename.as_ref().map(|p| p.as_ref()).unwrap_or(Path::new("xcp_client_autodetect.a2l"));

        // Upload the A2L via XCP
        // Be aware the file name may be the original A2L file written by registry
        if filename.is_none() {
            info!("Upload A2L to {}", a2l_filename.display());
            {
                let file = std::fs::File::create(a2l_filename)?;
                let mut writer = std::io::BufWriter::new(file);
                let (file_size, _) = self.get_id(XCP_IDT_ASAM_UPLOAD).await?;
                assert!(file_size > 0);
                let mut size = file_size;
                while size > 0 {
                    let n = if size > 200 { 200 } else { size as u8 };
                    size -= n as u32;
                    let data = self.upload(n).await?;
                    trace!("xcp_client.upload: {} bytes = {:?}", data.len(), data);
                    writer.write_all(&data[1..=n as usize])?;
                }
                writer.flush()?;
                info!("  Upload complete, {} bytes loaded", file_size);
            }
        }

        // Read the A2L file
        //info!("Read A2L {}", a2l_filename.display());
        if let Ok(a2l_file) = a2l_load(a2l_filename) {
            if print_info {
                a2l_printf_info(&a2l_file);
            }
            self.a2l_file = Some(a2l_file);
        } else {
            error!("Could not read A2L file {}", a2l_filename.display());
            return Err(Box::new(XcpError::new(ERROR_A2L, 0)) as Box<dyn Error>);
        }

        Ok(())
    }

    pub fn get_a2l_file(&self) -> Option<&a2lfile::A2lFile> {
        self.a2l_file.as_ref()
    }

    //------------------------------------------------------------------------
    // A2l

    pub fn get_characteristics(&self) -> Vec<String> {
        a2l_get_characteristics(self.a2l_file.as_ref().unwrap())
    }

    pub fn get_measurements(&self) -> Vec<String> {
        a2l_get_measurements(self.a2l_file.as_ref().unwrap())
    }

    //------------------------------------------------------------------------
    // XcpCalibrationObject, XcpCalibrationObjectHandle (index pointer to XcpCalibrationObject),
    // XcpXcpCalibrationObjectHandle is assumed immutable and the actual value is cached

    pub fn get_calibration_object(&mut self, handle: XcpCalibrationObjectHandle) -> &XcpCalibrationObject {
        &self.calibration_objects[handle.0]
    }

    pub async fn create_calibration_object(&mut self, name: &str) -> Result<XcpCalibrationObjectHandle, Box<dyn Error>> {
        let res = a2l_find_characteristic(self.a2l_file.as_ref().unwrap(), name);
        if res.is_none() {
            debug!("create_calibration_object: characteristic {} not found", name);
            Err(Box::new(XcpError::new(ERROR_A2L, 0)) as Box<dyn Error>)
        } else {
            let (a2l_addr, a2l_type, a2l_limits) = res.unwrap();

            let mut o = XcpCalibrationObject::new(name, a2l_addr, a2l_type, a2l_limits);
            let resp = self.short_upload(o.a2l_addr.addr, o.a2l_addr.ext, o.get_type.size).await?;
            o.value = resp[1..=o.get_type.size as usize].to_vec();
            trace!("upload {}: addr = {:?} type = {:?} limit={:?} value={:?}\n", name, a2l_addr, a2l_type, a2l_limits, o.value);
            self.calibration_objects.push(o);
            Ok(XcpCalibrationObjectHandle(self.calibration_objects.len() - 1))
        }
    }

    pub async fn set_value_u64(&mut self, handle: XcpCalibrationObjectHandle, value: u64) -> Result<(), Box<dyn Error>> {
        let obj = &self.calibration_objects[handle.0];
        if (value as f64) > obj.a2l_limits.upper || (value as f64) < obj.a2l_limits.lower {
            return Err(Box::new(XcpError::new(ERROR_LIMIT, 0)) as Box<dyn Error>);
        }
        let size: usize = obj.get_type.size as usize;
        let slice = &value.to_le_bytes()[0..size];
        self.short_download(obj.a2l_addr.addr, obj.a2l_addr.ext, slice).await?;
        self.calibration_objects[handle.0].set_value(slice);
        Ok(())
    }
    pub async fn set_value_i64(&mut self, handle: XcpCalibrationObjectHandle, value: i64) -> Result<(), Box<dyn Error>> {
        let obj = &self.calibration_objects[handle.0];
        if (value as f64) > obj.a2l_limits.upper || (value as f64) < obj.a2l_limits.lower {
            return Err(Box::new(XcpError::new(ERROR_LIMIT, 0)) as Box<dyn Error>);
        }
        let size: usize = obj.get_type.size as usize;
        let slice = &value.to_le_bytes()[0..size];
        self.short_download(obj.a2l_addr.addr, obj.a2l_addr.ext, slice).await?;
        self.calibration_objects[handle.0].set_value(slice);
        Ok(())
    }
    pub async fn set_value_f64(&mut self, handle: XcpCalibrationObjectHandle, value: f64) -> Result<(), Box<dyn Error>> {
        let obj = &self.calibration_objects[handle.0];
        if value > obj.a2l_limits.upper || value < obj.a2l_limits.lower {
            return Err(Box::new(XcpError::new(ERROR_LIMIT, 0)) as Box<dyn Error>);
        }
        let size: usize = obj.get_type.size as usize;
        let slice = &value.to_le_bytes()[0..size];
        self.short_download(obj.a2l_addr.addr, obj.a2l_addr.ext, slice).await?;
        self.calibration_objects[handle.0].set_value(slice);
        Ok(())
    }

    pub async fn read_value_u64(&mut self, index: XcpCalibrationObjectHandle) -> Result<u64, Box<dyn Error>> {
        let a2l_addr = self.calibration_objects[index.0].a2l_addr;
        let get_type = self.calibration_objects[index.0].get_type;
        let resp = self.short_upload(a2l_addr.addr, a2l_addr.ext, get_type.size).await?;
        let value = resp[1..=get_type.size as usize].to_vec();
        self.calibration_objects[index.0].value = value;
        Ok(self.get_value_u64(index))
    }

    pub fn get_value_u64(&mut self, index: XcpCalibrationObjectHandle) -> u64 {
        let obj = &self.calibration_objects[index.0];
        obj.get_value_u64()
    }

    pub fn get_value_i64(&mut self, index: XcpCalibrationObjectHandle) -> i64 {
        let obj = &self.calibration_objects[index.0];
        obj.get_value_i64()
    }
    pub fn get_value_f64(&mut self, index: XcpCalibrationObjectHandle) -> f64 {
        let obj = &self.calibration_objects[index.0];
        let v = obj.get_value_u64();
        #[allow(clippy::transmute_int_to_float)]
        // @@@@ - unsafe - Test XCP client
        unsafe {
            std::mem::transmute(v)
        }
    }

    //------------------------------------------------------------------------
    // XcpMeasurementObject, XcpMeasurmentObjectHandle (index pointer to XcpCMeasurmentObject),
    //

    pub fn create_measurement_object(&mut self, name: &str) -> Option<XcpMeasurementObjectHandle> {
        let (a2l_addr, a2l_type) = a2l_find_measurement(self.a2l_file.as_ref().unwrap(), name)?;
        let o = XcpMeasurementObject::new(name, a2l_addr, a2l_type);
        debug!("Create measurement object {}: addr = {:?} type = {:?}", name, a2l_addr, a2l_type,);
        self.measurement_objects.push(o);
        Some(XcpMeasurementObjectHandle(self.measurement_objects.len() - 1))
    }

    //------------------------------------------------------------------------
    // DAQ init, start, stop
    //

    /// Get clock resolution in ns
    pub fn get_timestamp_resolution(&self) -> u64 {
        self.timestamp_resolution_ns
    }

    /// Start DAQ
    pub async fn start_measurement(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Start measurement");

        // Init
        let signal_count = self.measurement_objects.len();
        let mut daq_odt_entries: Vec<Vec<OdtEntry>> = Vec::with_capacity(8);

        // Store all events in a hashmap (eventnumber, signalcount)
        let mut event_map: HashMap<u16, u16> = HashMap::new();
        let mut min_event: u16 = 0xFFFF;
        let mut max_event: u16 = 0;
        for i in 0..signal_count {
            let event = self.measurement_objects[i].get_addr().event;
            if event < min_event {
                min_event = event;
            }
            if event > max_event {
                max_event = event;
            }
            let count = event_map.entry(event).or_insert(0);
            *count += 1;
        }
        let event_count: u16 = event_map.len() as u16;
        info!("event/daq count = {}", event_count);

        // Transform the event hashmap to a sorted array
        let mut event_list: Vec<(u16, u16)> = Vec::new();
        for (event, count) in event_map.into_iter() {
            event_list.push((event, count));
        }
        event_list.sort_by(|a, b| a.0.cmp(&b.0));

        // Alloc a DAQ list for each event
        assert!(event_count <= 1024, "event_count > 1024");
        let daq_count: u16 = event_count;
        self.free_daq().await?;
        self.alloc_daq(daq_count).await?;
        debug!("alloc_daq count={}", daq_count);

        // Alloc one ODT for each DAQ list (event)
        // @@@@ Restriction: Only one ODT per DAQ list supported yet
        for daq in 0..daq_count {
            self.alloc_odt(daq, 1).await?;
            debug!("Alloc daq={}, odt_count={}", daq, 1);
        }

        // Alloc ODT entries (signal count) for each ODT/DAQ list
        for daq in 0..daq_count {
            let odt_entry_count = event_list[daq as usize].1;
            assert!(odt_entry_count < 0x7C, "odt_entry_count >= 0x7C");
            self.alloc_odt_entries(daq, 0, odt_entry_count as u8).await?;
            debug!("Alloc odt_entries: daq={}, odt={}, odt_entry_count={}", daq, 0, odt_entry_count);
        }

        // Create all ODT entries for each daq/event list and store information for the DAQ decoder
        for daq in 0..daq_count {
            //
            let event = event_list[daq as usize].0;
            let odt = 0; // Only one odt per daq list supported yet
            let odt_entry_count = self.measurement_objects.len();

            // Create ODT entries for this daq list
            let mut odt_entries = Vec::new();
            let mut odt_size: u16 = 0;
            self.set_daq_ptr(daq, odt, 0).await?;
            for odt_entry in 0..odt_entry_count {
                let m = &mut self.measurement_objects[odt_entry];
                let a2l_addr = m.a2l_addr;
                if a2l_addr.event == event {
                    // Only add signals for the daq list event
                    let a2l_type: A2lType = m.a2l_type;
                    m.daq = daq;
                    m.odt = odt;
                    m.offset = odt_size + 6;

                    debug!(
                        "WRITE_DAQ {} daq={}, odt={},  type={:?}, size={}, ext={}, addr=0x{:08X}, offset={}",
                        m.name,
                        daq,
                        odt,
                        a2l_type.encoding,
                        a2l_type.size,
                        a2l_addr.ext,
                        a2l_addr.addr,
                        odt_size + 6
                    );

                    odt_entries.push(OdtEntry {
                        name: m.name.clone(),
                        a2l_type,
                        a2l_addr,
                        offset: odt_size,
                    });

                    self.write_daq(a2l_addr.ext, a2l_addr.addr, a2l_type.size).await?;

                    odt_size += a2l_type.size as u16;
                    if odt_size > self.max_dto_size - 6 {
                        return Err(Box::new(XcpError::new(ERROR_ODT_SIZE, 0)) as Box<dyn Error>);
                    }
                }
            } // odt_entries

            daq_odt_entries.push(odt_entries);
        }

        // Set DAQ list events
        for daq in 0..daq_count {
            let event = event_list[daq as usize].0;
            self.set_daq_list_mode(daq, event).await?;
            debug!("Set event: daq={}, event={}", daq, event);
        }

        // Select and prepare all DAQ lists
        for daq in 0..daq_count {
            self.select_daq_list(daq).await?;
        }
        self.prepare_selected_daq_lists().await?;

        // Reset the DAQ decoder and set measurement start time
        let daq_clock = self.get_daq_clock_raw().await?;
        self.daq_decoder.as_ref().unwrap().lock().start(daq_odt_entries, daq_clock);

        // Send running=true throught the DAQ control channel to the receive task
        self.task_control.running = true;
        self.tx_task_control.as_ref().unwrap().send(self.task_control).await.unwrap();

        // Start DAQ
        self.start_selected_daq_lists().await?;

        Ok(())
    }

    /// Stop DAQ
    pub async fn stop_measurement(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Stop measurement");

        // Stop DAQ
        let res = self.stop_all_daq_lists().await;

        // Send running=false throught the DAQ control channel to the receive task
        self.task_control.running = false;
        self.tx_task_control.as_ref().unwrap().send(self.task_control).await?;

        // Stop the DAQ decoder
        self.daq_decoder.as_ref().unwrap().lock().stop();

        res
    }
}
