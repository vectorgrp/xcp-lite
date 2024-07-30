//--------------------------------------------------------------------------------------------------------------------------------------------------
// Module xcp_client
// Simplified, quick and dirty implementation of an UDP XCP client for integration testing

#![allow(dead_code)] // because of all the unused XCP definitions

//#![allow(unused_imports)]

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use bytes::{BufMut, BytesMut};
use std::collections::HashMap;
use std::error::Error;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::time::{timeout, Duration};

#[allow(unused_imports)]
use crate::a2l::a2l_reader::{
    a2l_find_characteristic, a2l_find_measurement, a2l_load, a2l_printf_info, A2lAddr, A2lLimits,
    A2lType,
};

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP Parameters

const CMD_TIMEOUT: Duration = Duration::from_secs(1);

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

pub struct XcpError {
    code: u8,
}

impl XcpError {
    pub fn new(code: u8) -> XcpError {
        XcpError { code }
    }
    pub fn get_error_code(&self) -> u8 {
        self.code
    }
}

impl std::fmt::Display for XcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.code {
            ERROR_CMD_TIMEOUT => {
                write!(f, "Command response timeout")
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
                write!(f, "XCP command IGNORED")
            }
            CRC_CMD_BUSY => {
                write!(f, "XCP command BUSY")
            }
            CRC_DAQ_ACTIVE => {
                write!(f, "XCP DAQ ACTIVE")
            }
            CRC_PRM_ACTIVE => {
                write!(f, "XCP PRM ACTIVE")
            }
            CRC_CMD_UNKNOWN => {
                write!(f, "XCP command UNKNOWN")
            }
            CRC_CMD_SYNTAX => {
                write!(f, "XCP command SYNTAX")
            }
            CRC_OUT_OF_RANGE => {
                write!(f, "Parameter out of range")
            }
            CRC_WRITE_PROTECTED => {
                write!(f, "Write protected")
            }
            CRC_ACCESS_DENIED => {
                write!(f, "Access denied")
            }
            CRC_ACCESS_LOCKED => {
                write!(f, "Access locked")
            }
            CRC_PAGE_NOT_VALID => {
                write!(f, "Invalid page")
            }
            CRC_PAGE_MODE_NOT_VALID => {
                write!(f, "XInvalide page mode")
            }
            CRC_SEGMENT_NOT_VALID => {
                write!(f, "Invalid segment")
            }
            CRC_SEQUENCE => {
                write!(f, "Wrong sequence")
            }
            CRC_DAQ_CONFIG => {
                write!(f, "DAQ configuration error")
            }
            CRC_MEMORY_OVERFLOW => {
                write!(f, "Memory overflow")
            }
            CRC_GENERIC => {
                write!(f, "XCP generic error")
            }
            CRC_VERIFY => {
                write!(f, "Verify failed")
            }
            CRC_RESOURCE_TEMPORARY_NOT_ACCESSIBLE => {
                write!(f, "Resource temporary not accessible")
            }
            CRC_SUBCMD_UNKNOWN => {
                write!(f, "Unknown sub command")
            }
            CRC_TIMECORR_STATE_CHANGE => {
                write!(f, "Time correlation state change")
            }
            _ => {
                write!(f, "XCP error code = 0x{:0X}", self.code)
            }
        }
    }
}

impl std::fmt::Debug for XcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "XcpError 0x{:04X}", self.code)
    }
}

impl std::error::Error for XcpError {}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XCP protocol definitions

// XCP command codes
pub const CC_CONNECT: u8 = 0xFF;
pub const CC_DISCONNECT: u8 = 0xFE;
pub const CC_SHORT_DOWNLOAD: u8 = 0xED;
pub const CC_UPLOAD: u8 = 0xF5;
pub const CC_SHORT_UPLOAD: u8 = 0xF4;
pub const CC_SYNC: u8 = 0xFC;
pub const CC_NOP: u8 = 0xC1;
pub const CC_GET_ID: u8 = 0xFA;
pub const CC_GET_CAL_PAGE: u8 = 0xEA;
pub const CC_SET_CAL_PAGE: u8 = 0xEB;
pub const CC_GET_DAQ_PROCESSOR_INFO: u8 = 0xE9;
pub const CC_GET_SEGMENT_INFO: u8 = 0xE8;
pub const CC_GET_PAGE_INFO: u8 = 0xE7;
pub const CC_SET_SEGMENT_MODE: u8 = 0xE6;
pub const CC_GET_SEGMENT_MODE: u8 = 0xE5;
pub const CC_COPY_CAL_PAGE: u8 = 0xE4;

pub const CC_ALLOC_ODT: u8 = 0xD4;
pub const CC_ALLOC_ODT_ENTRY: u8 = 0xD3;
pub const CC_SET_DAQ_LIST_MODE: u8 = 0xE0;
pub const CC_READ_DAQ: u8 = 0xDB;
pub const CC_CLEAR_DAQ_LIST: u8 = 0xE3;
pub const CC_SET_DAQ_PTR: u8 = 0xE2;
pub const CC_WRITE_DAQ: u8 = 0xE1;
pub const CC_GET_DAQ_LIST_MODE: u8 = 0xDF;
pub const CC_START_STOP_DAQ_LIST: u8 = 0xDE;
pub const CC_START_STOP_SYNCH: u8 = 0xDD;
pub const CC_GET_DAQ_CLOCK: u8 = 0xDC;
pub const CC_GET_DAQ_RESOLUTION_INFO: u8 = 0xD9;
pub const CC_GET_DAQ_LIST_INFO: u8 = 0xD8;
pub const CC_GET_DAQ_EVENT_INFO: u8 = 0xD7;
pub const CC_FREE_DAQ: u8 = 0xD6;
pub const CC_ALLOC_DAQ: u8 = 0xD5;

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
        let mut len = self.data.len() as u16;
        assert!(len >= 5);
        len -= 4;
        self.data[0] = len as u8;
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
    pub fn new(
        name: &str,
        a2l_addr: A2lAddr,
        get_type: A2lType,
        a2l_limits: A2lLimits,
    ) -> XcpCalibrationObject {
        XcpCalibrationObject {
            name: name.to_string(),
            a2l_addr,
            get_type,
            a2l_limits,
            value: Vec::new(),
        }
    }

    pub fn set_value(&mut self, bytes: &[u8]) {
        self.value = bytes.to_vec();
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
    pub get_type: A2lType,
    pub daq: u8,
    pub odt: u8,
    pub offset: u16,
}

impl XcpMeasurementObject {
    pub fn new(name: &str, a2l_addr: A2lAddr, get_type: A2lType) -> XcpMeasurementObject {
        XcpMeasurementObject {
            name: name.to_string(),
            a2l_addr,
            get_type,
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
        self.get_type
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Default printf decoder for XCP SERV_TEXT data

pub trait XcpTextDecoder {
    fn decode(&self, data: &[u8]);
}

struct DefaultTextDecoder;

impl DefaultTextDecoder {
    pub fn new() -> DefaultTextDecoder {
        DefaultTextDecoder {}
    }
}

impl XcpTextDecoder for DefaultTextDecoder {
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

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Default printf decoder for XCP DAQ data

pub trait XcpDaqDecoder {
    fn decode(&mut self, control: &XcpTaskControl, data: &[u8]);
}

struct DefaultDaqDecoder;

impl DefaultDaqDecoder {
    pub fn new() -> DefaultDaqDecoder {
        DefaultDaqDecoder {}
    }
}

impl XcpDaqDecoder for DefaultDaqDecoder {
    // Handle incomming DAQ data from XCP server
    fn decode(&mut self, control: &XcpTaskControl, data: &[u8]) {
        if control.running && control.connected {
            let mut daq = data[1];
            if (daq & 0x80) != 0 {
                error!("DAQ queue overflow!");
                daq &= 0x7F;
            }
            let odt = data[0];
            if odt == 0 {
                let timestamp = data[2] as u32
                    | (data[3] as u32) << 8
                    | (data[4] as u32) << 16
                    | (data[5] as u32) << 24;

                println!(
                    "DAQ: daq = {}, odt = {} timestamp = {} data={:?})",
                    daq,
                    odt,
                    timestamp,
                    &data[6..]
                );
            } else {
                panic!("ODT != 0");
            }
        }
    }
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
        XcpTaskControl {
            running: false,
            connected: false,
        }
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// XcpClient type

/// XCP client
pub struct XcpClient {
    bind_addr: SocketAddr,
    dest_addr: SocketAddr,
    socket: Option<Arc<UdpSocket>>,

    rx_cmd_resp: Option<mpsc::Receiver<Vec<u8>>>,
    tx_task_control: Option<mpsc::Sender<XcpTaskControl>>,
    task_control: XcpTaskControl,

    ctr: u16,
    max_cto_size: u8,
    max_dto_size: u16,

    a2l_file: Option<a2lfile::A2lFile>,
    calibration_objects: Vec<XcpCalibrationObject>,
    measurement_objects: Vec<XcpMeasurementObject>,
}

impl XcpClient {
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
        let mut buf: [u8; 8000] = [0; 8000]; // @@@@ Impl:
        let mut task_control: Option<XcpTaskControl> = None;

        loop {
            select! {

                // Handle the data from rx_daq_decoder
                res = rx_daq_decoder.recv() => {
                    match res {
                        Some(c) => {
                            debug!("receive_task: new daq decoder received connected={} running={}", c.connected, c.running);
                            if !c.connected { // Handle the data from rx_daq_decoder
                                debug!("receive_task: return, disconnected");
                                return Ok(());
                            }
                            task_control = Some(c);
                        }
                        None => { // The sender has been dropped
                            debug!("receive_task: return, channel closed");
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
                                    return Err(Box::new(XcpError::new(ERROR_TL_HEADER)) as Box<dyn Error>);
                                }
                                let len = buf[i] as usize + ((buf[i + 1] as usize) << 8);
                                if len > size - 4 || len == 0 {
                                    return Err(Box::new(XcpError::new(ERROR_TL_HEADER)) as Box<dyn Error>);
                                }
                                let ctr = buf[i + 2] as u16 + ((buf[i + 3] as u16) << 8);
                                if ctr_first {
                                    ctr_first = false;
                                } else if ctr != ctr_last.wrapping_add(1) {
                                    error!(
                                        "xcp_receive: missing packet, last_ctr = {}, ctr = {}",
                                        ctr_last, ctr
                                    );
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
                                        trace!("xcp_receive: XCP errorcode = 0x{:?} ", response);
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
                                                "xcp_receive: unknown service request code = 0x{:0X} ignored",
                                                service_code
                                            );
                                        }
                                    }
                                    _ => {
                                        // Check that we got a DAQ control
                                        if let Some(c) = &task_control {

                                            // Handle DAQ data if DAQ running
                                            if c.running {
                                                let mut m = decode_daq.lock().unwrap();
                                                m.decode(c, &buf[i + 4..i + 4 + len]);
                                            }
                                        }
                                    }
                                } // match pid
                                i = i + len + 4;
                            } // while message in packet


                        }
                        Err(e) => {
                            // Handle the error from recv_from
                            error!("xcp_receive: socket error {}",e);
                            return Err(Box::new(XcpError::new(ERROR_TL_HEADER)) as Box<dyn Error>);
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
                                Err(Box::new(XcpError::new(data[1])) as Box<dyn Error>)
                            }
                            _ => {
                                panic!("xcp_command: bug in receive_task");
                            }
                        }
                    }
                    None => {
                        panic!("xcp_command: bug in receive_task, no data in channel")
                    }
                }
            }
            Err(_) => {
                // Timeout, return with XcpError
                Err(Box::new(XcpError::new(ERROR_CMD_TIMEOUT)) as Box<dyn Error>)
            }
        }
    }

    //------------------------------------------------------------------------
    // Connect/disconnect to server, create receive task

    pub async fn connect<D, T>(
        &mut self,
        daq_decoder: Arc<Mutex<D>>,
        text_decoder: T,
    ) -> Result<(), Box<dyn Error>>
    where
        T: XcpTextDecoder + Copy + Send + 'static,
        D: XcpDaqDecoder + Copy + Send + 'static,
    {
        // Create socket
        let socket = UdpSocket::bind(self.bind_addr).await?;
        self.socket = Some(Arc::new(socket));

        // Spawn a task to handle incomming data

        {
            let socket = Arc::clone(self.socket.as_ref().unwrap());
            let (tx_resp, rx_resp) = mpsc::channel(1);
            self.rx_cmd_resp = Some(rx_resp); // rx XCP command response channel
            let (tx_daq, rx_daq) = mpsc::channel(3);
            self.tx_task_control = Some(tx_daq); // tx XCP DAQ control channel
            tokio::spawn(async move {
                let _res =
                    XcpClient::receive_task(socket, tx_resp, rx_daq, text_decoder, daq_decoder)
                        .await;
            });
        }

        let data = self
            .send_command(XcpCommandBuilder::new(CC_CONNECT).add_u8(0).build())
            .await?;

        let max_cto_size: u8 = data[3];
        let max_dto_size: u16 = data[4] as u16 | (data[5] as u16) << 8;
        info!(
            "XCP client connected, max_cto_size = {}, max_dto_size = {}",
            max_cto_size, max_dto_size
        );
        self.max_cto_size = max_cto_size;
        self.max_dto_size = max_dto_size;

        self.task_control.connected = true; // the task will end, when it gets connected = false over the XcpControl channel
        self.task_control.running = false;
        self.tx_task_control
            .as_ref()
            .unwrap()
            .send(self.task_control)
            .await
            .unwrap();

        Ok(())
    }

    //------------------------------------------------------------------------
    pub async fn disconnect(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_DISCONNECT).add_u8(0).build())
            .await?;

        self.task_control.connected = false;
        self.task_control.running = false;
        self.tx_task_control
            .as_ref()
            .unwrap()
            .send(self.task_control)
            .await
            .unwrap();

        Ok(())
    }

    //------------------------------------------------------------------------
    // Get server identification
    // @@@@ Impl: other types, only  XCP_IDT_ASAM_UPLOAD supported
    pub async fn get_id(&mut self, id_type: u8) -> Result<u32, Box<dyn Error>> {
        let data = self
            .send_command(XcpCommandBuilder::new(CC_GET_ID).add_u8(id_type).build())
            .await?;

        assert_eq!(data[0], 0xFF);
        assert_eq!(id_type, XCP_IDT_ASAM_UPLOAD); // others not supported yet
        let mode = data[1]; // 0 = data by upload, 1 = data in response
        assert_eq!(mode, 0);
        let mut size = 0u32;
        for i in (4..8).rev() {
            size = size << 8 | data[i] as u32;
        }
        trace!("GET_ID mode={} -> size = {}", id_type, size);
        Ok(size)
    }

    //------------------------------------------------------------------------
    // Execute a XCP command with no other parameters
    pub async fn command(&mut self, command_code: u8) -> Result<Vec<u8>, Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(command_code).build())
            .await
    }

    //------------------------------------------------------------------------
    // calibration segment and page control

    pub async fn get_ecu_page(&mut self) -> Result<u8, Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_ECU | 0x80;
        let segment = 0;
        let data = self
            .send_command(
                XcpCommandBuilder::new(CC_GET_CAL_PAGE)
                    .add_u8(mode)
                    .add_u8(segment)
                    .build(),
            )
            .await?;
        let page = if data[3] != 0 { 1 } else { 0 };
        Ok(page)
    }

    pub async fn get_xcp_page(&mut self) -> Result<u8, Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_XCP | 0x80;
        let segment = 0;
        let data = self
            .send_command(
                XcpCommandBuilder::new(CC_GET_CAL_PAGE)
                    .add_u8(mode)
                    .add_u8(segment)
                    .build(),
            )
            .await?;
        let page = if data[3] != 0 { 1 } else { 0 };
        Ok(page)
    }

    pub async fn set_ecu_page(&mut self, page: u8) -> Result<(), Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_ECU | 0x80;
        let segment = 0;
        self.send_command(
            XcpCommandBuilder::new(CC_SET_CAL_PAGE)
                .add_u8(mode)
                .add_u8(segment)
                .add_u8(page)
                .build(),
        )
        .await?;
        Ok(())
    }

    pub async fn set_xcp_page(&mut self, page: u8) -> Result<(), Box<dyn Error>> {
        let mode = CAL_PAGE_MODE_XCP | 0x80;
        let segment = 0;
        self.send_command(
            XcpCommandBuilder::new(CC_SET_CAL_PAGE)
                .add_u8(mode)
                .add_u8(segment)
                .add_u8(page)
                .build(),
        )
        .await?;
        Ok(())
    }

    //------------------------------------------------------------------------
    // XCP memory access services (calibration and polling of measurememt vvalues)

    pub async fn short_download(
        &mut self,
        addr: u32,
        ext: u8,
        data_bytes: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let len = data_bytes.len() as u8;
        trace!(
            "short_download addr={}:{:08X},{} data={:?}",
            ext,
            addr,
            len,
            data_bytes
        );
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
    pub async fn short_upload(
        &mut self,
        addr: u32,
        ext: u8,
        size: u8,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let data = self
            .send_command(
                XcpCommandBuilder::new(CC_SHORT_UPLOAD)
                    .add_u8(size)
                    .add_u8(0)
                    .add_u8(ext)
                    .add_u32(addr)
                    .build(),
            )
            .await?;

        Ok(data)
    }

    pub async fn upload(&mut self, size: u8) -> Result<Vec<u8>, Box<dyn Error>> {
        let data = self
            .send_command(XcpCommandBuilder::new(CC_UPLOAD).add_u8(size).build())
            .await?;
        Ok(data)
    }

    //------------------------------------------------------------------------
    // XCP DAQ services

    async fn free_daq(&mut self) -> Result<(), Box<dyn Error>> {
        self.send_command(XcpCommandBuilder::new(CC_FREE_DAQ).build())
            .await?;
        Ok(())
    }

    async fn alloc_daq(&mut self, count: u16) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_ALLOC_DAQ)
                .add_u8(0)
                .add_u16(count)
                .build(),
        )
        .await?;
        Ok(())
    }

    async fn alloc_odt(&mut self, daq: u16, odt: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_ALLOC_ODT)
                .add_u8(0)
                .add_u16(daq)
                .add_u8(odt)
                .build(),
        )
        .await?;
        Ok(())
    }

    async fn alloc_odt_entries(
        &mut self,
        daq: u16,
        odt: u8,
        count: u8,
    ) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_ALLOC_ODT_ENTRY)
                .add_u8(0)
                .add_u16(daq)
                .add_u8(odt)
                .add_u8(count)
                .build(),
        )
        .await?;
        Ok(())
    }

    async fn set_daq_ptr(&mut self, daq: u16, odt: u8, idx: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_SET_DAQ_PTR)
                .add_u8(0)
                .add_u16(daq)
                .add_u8(odt)
                .add_u8(idx)
                .build(),
        )
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

    async fn set_daq_list_mode(
        &mut self,
        daq: u16,
        eventchannel: u16,
    ) -> Result<(), Box<dyn Error>> {
        const XCP_DAQ_MODE_TIMESTAMP: u8 = 0x10; // Timestamp always on, no other mode supported by XCPlite
        let mode: u8 = XCP_DAQ_MODE_TIMESTAMP;
        let priority = 0x00; // Always use priority 0, not DAQ list flushing for specific events, priorization supported by XCPlite
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

    // START_STOP mode
    const XCP_STOP: u8 = 0;
    const XCP_START: u8 = 1;
    const XCP_SELECT: u8 = 2;
    async fn start_stop_daq_list(&mut self, mode: u8, daq: u16) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_START_STOP_DAQ_LIST)
                .add_u8(mode)
                .add_u16(daq)
                .build(),
        )
        .await?;
        Ok(())
    }

    // START_STOP_SYNC mode
    const XCP_STOP_ALL: u8 = 0;
    const XCP_START_SELECTED: u8 = 1;
    const XCP_STOP_SELECTED: u8 = 2;
    const XCP_PREPARE_START_SELECTED: u8 = 3;
    async fn start_stop_sync(&mut self, mode: u8) -> Result<(), Box<dyn Error>> {
        self.send_command(
            XcpCommandBuilder::new(CC_START_STOP_SYNCH)
                .add_u8(mode)
                .build(),
        )
        .await?;
        Ok(())
    }

    //-------------------------------------------------------------------------------------------------
    // A2L upload

    // Upload the A2L via XCP and load it
    pub async fn upload_a2l(&mut self) -> Result<(), Box<dyn Error>> {
        // Upload the A2L via XCP
        info!("Upload A2L xcp_client.a2l");
        std::fs::remove_file("xcp_client.a2l").ok();
        {
            let file = std::fs::File::create("xcp_client.a2l").unwrap();
            let mut writer = std::io::BufWriter::new(file);
            let mut size = self.get_id(XCP_IDT_ASAM_UPLOAD).await?;
            while size > 0 {
                let n = if size > 200 { 200 } else { size as u8 };
                size -= n as u32;
                let data = self.upload(n).await?;
                trace!("xcp_client.upload: {} bytes = {:?}", data.len(), data);
                writer.write_all(&data[1..]).unwrap();
            }
        }

        // Read the uploaded A2L file
        info!("Read A2L xcp_client.a2l");
        if let Ok(a2l_file) = a2l_load("xcp_client.a2l") {
            //a2l_printf_info(&a2l_file);
            self.a2l_file = Some(a2l_file);
        } else {
            return Err(Box::new(XcpError::new(ERROR_A2L)) as Box<dyn Error>);
        }

        Ok(())
    }

    pub fn get_a2l_file(&self) -> Option<&a2lfile::A2lFile> {
        self.a2l_file.as_ref()
    }

    //------------------------------------------------------------------------
    // XcpCalibrationObject, XcpCalibrationObjectHandle (index pointer to XcpCalibrationObject),
    // XcpXcpCalibrationObjectHandle is assumed immutable and the actual value is cached

    pub async fn create_calibration_object(
        &mut self,
        name: &str,
    ) -> Result<XcpCalibrationObjectHandle, Box<dyn Error>> {
        let res = a2l_find_characteristic(self.a2l_file.as_ref().unwrap(), name);
        if res.is_none() {
            Err(Box::new(XcpError::new(ERROR_A2L)) as Box<dyn Error>)
        } else {
            let (a2l_addr, get_type, a2l_limits) = res.unwrap();

            let mut o = XcpCalibrationObject::new(name, a2l_addr, get_type, a2l_limits);
            let resp = self
                .short_upload(o.a2l_addr.addr, o.a2l_addr.ext, o.get_type.size)
                .await?;
            o.value = resp[1..=o.get_type.size as usize].to_vec();
            trace!(
                "upload {}: addr = {:?} type = {:?} limit={:?} value={:?}\n",
                name,
                a2l_addr,
                get_type,
                a2l_limits,
                o.value
            );
            self.calibration_objects.push(o);
            Ok(XcpCalibrationObjectHandle(
                self.calibration_objects.len() - 1,
            ))
        }
    }

    pub async fn set_value_u64(
        &mut self,
        handle: XcpCalibrationObjectHandle,
        value: u64,
    ) -> Result<(), Box<dyn Error>> {
        let obj = &self.calibration_objects[handle.0];
        if (value as f64) > obj.a2l_limits.upper || (value as f64) < obj.a2l_limits.lower {
            return Err(Box::new(XcpError::new(ERROR_LIMIT)) as Box<dyn Error>);
        }
        let size: usize = obj.get_type.size as usize;
        let slice = &value.to_le_bytes()[0..size];
        self.short_download(obj.a2l_addr.addr, obj.a2l_addr.ext, slice)
            .await?;
        self.calibration_objects[handle.0].set_value(slice);
        Ok(())
    }
    pub async fn set_value_i64(
        &mut self,
        handle: XcpCalibrationObjectHandle,
        value: i64,
    ) -> Result<(), Box<dyn Error>> {
        let obj = &self.calibration_objects[handle.0];
        if (value as f64) > obj.a2l_limits.upper || (value as f64) < obj.a2l_limits.lower {
            return Err(Box::new(XcpError::new(ERROR_LIMIT)) as Box<dyn Error>);
        }
        let size: usize = obj.get_type.size as usize;
        let slice = &value.to_le_bytes()[0..size];
        self.short_download(obj.a2l_addr.addr, obj.a2l_addr.ext, slice)
            .await?;
        self.calibration_objects[handle.0].set_value(slice);
        Ok(())
    }

    pub async fn read_value_u64(
        &mut self,
        index: XcpCalibrationObjectHandle,
    ) -> Result<u64, Box<dyn Error>> {
        let a2l_addr = self.calibration_objects[index.0].a2l_addr;
        let get_type = self.calibration_objects[index.0].get_type;

        let resp = self
            .short_upload(a2l_addr.addr, a2l_addr.ext, get_type.size)
            .await?;

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

    //------------------------------------------------------------------------
    // XcpMeasurmentObject, XcpMeasurmentObjectHandle (index pointer to XcpCMeasurmentObject),
    //

    pub fn create_measurement_object(&mut self, name: &str) -> Option<XcpMeasurementObjectHandle> {
        let (a2l_addr, get_type) = a2l_find_measurement(self.a2l_file.as_ref().unwrap(), name)?;
        let o = XcpMeasurementObject::new(name, a2l_addr, get_type);
        debug!(
            "Create measurement object {}: addr = {:?} type = {:?}",
            name, a2l_addr, get_type,
        );
        self.measurement_objects.push(o);
        Some(XcpMeasurementObjectHandle(
            self.measurement_objects.len() - 1,
        ))
    }

    //------------------------------------------------------------------------
    // DAQ start, stop
    //

    pub async fn start_measurement(&mut self) -> Result<(), Box<dyn Error>> {
        let n = self.measurement_objects.len();

        // Find all  events
        let mut event_list: HashMap<u16, u16> = HashMap::new();
        for i in 0..n {
            let event = self.measurement_objects[i].get_addr().event;
            let count = event_list.entry(event).or_insert(0);
            *count += 1;
        }
        debug!("event_list = {:?}", event_list);
        let event_count: u16 = event_list.len() as u16;
        debug!("event_count = {}", event_count);

        // Alloc DAQ lists
        // @@@@ Restriction: Only one DAQ list per event supported
        // @@@@ Restriction: Maximal 256 DAQ lists
        assert!(event_count <= 256, "event_count > 256");
        let daq_count: u8 = event_count as u8;
        self.free_daq().await.unwrap();
        self.alloc_daq(daq_count as u16).await.unwrap();
        debug!("alloc_daq count={}", daq_count);

        // Alloc ODTs
        // @@@@ Restriction: Only one ODT per DAQ list supported
        for daq in 0..daq_count {
            self.alloc_odt(daq as u16, 1).await.unwrap();
            debug!("Alloc daq={}, odt_count={}", daq, 1);
        }

        // Alloc ODT entries
        for daq in 0..daq_count {
            let element = event_list.iter().nth(daq as usize).unwrap();
            let odt_entry_count = *element.1;
            assert!(odt_entry_count <= 0xFF);
            self.alloc_odt_entries(daq as u16, 0, odt_entry_count as u8)
                .await
                .unwrap();
            debug!(
                "Alloc odt_entries: daq={}, odt={}, odt_entry_count={}",
                daq, 0, odt_entry_count
            );
        }

        // Write ODT entries
        for daq in 0..daq_count {
            let odt = 0;
            let element = event_list.iter().nth(daq as usize).unwrap();
            let event = *element.0;
            let mut odt_entry: u8 = 0;
            let n = self.measurement_objects.len();
            let mut odt_size: u16 = 0;
            for i in 0..n {
                let a2l_addr = self.measurement_objects[i].a2l_addr;
                if a2l_addr.event == event {
                    self.set_daq_ptr(daq as u16, odt, odt_entry).await.unwrap();
                    let get_type = self.measurement_objects[i].get_type;
                    self.write_daq(a2l_addr.ext, a2l_addr.addr, get_type.size)
                        .await?;

                    self.measurement_objects[i].daq = daq;
                    self.measurement_objects[i].odt = odt;
                    self.measurement_objects[i].offset = odt_size + 6;

                    debug!(
                        "Write daq={}, odt={}, odt_entry={}, ext={}, addr=0x{:08X}, size={}, offset={}",
                        daq, odt, odt_entry, a2l_addr.ext, a2l_addr.addr, get_type.size, odt_size + 6
                    );

                    odt_entry += 1;
                    odt_size += get_type.size as u16;
                    if odt_size > self.max_dto_size - 6 {
                        return Err(Box::new(XcpError::new(ERROR_ODT_SIZE)) as Box<dyn Error>);
                    }
                }
            }
        }

        // Set DAQ list events
        for daq in 0..daq_count {
            let element = event_list.iter().nth(daq as usize).unwrap();
            let event = *element.0;
            self.set_daq_list_mode(daq as u16, event).await.unwrap();
            debug!("Set event: daq={}, event={}", daq, event);
        }

        // Select all DAQ lists
        for daq in 0..daq_count {
            self.start_stop_daq_list(XcpClient::XCP_SELECT, daq as u16)
                .await
                .unwrap();
        }

        // Send running=true throught the DAQ control channel to the receive task
        self.task_control.running = true;
        self.tx_task_control
            .as_ref()
            .unwrap()
            .send(self.task_control)
            .await
            .unwrap();

        // Start DAQ
        self.start_stop_sync(XcpClient::XCP_START_SELECTED)
            .await
            .unwrap();

        Ok(())
    }

    pub async fn stop_measurement(&mut self) -> Result<(), Box<dyn Error>> {
        // Stop DAQ
        self.start_stop_sync(XcpClient::XCP_STOP_ALL).await.unwrap();

        // Send running=false throught the DAQ control channel to the receive task
        self.task_control.running = false;
        self.tx_task_control
            .as_ref()
            .unwrap()
            .send(self.task_control)
            .await
            .unwrap();

        Ok(())
    }

    //------------------------------------------------------------------------
    // new
    //
    #[allow(clippy::type_complexity)] // clippy complaining about the measurment_list slice
    pub fn new(dest_addr: SocketAddr, bind_addr: SocketAddr) -> XcpClient {
        XcpClient {
            dest_addr,
            bind_addr,
            socket: None,
            ctr: 0,
            max_cto_size: 0,
            max_dto_size: 0,
            rx_cmd_resp: None,
            tx_task_control: None,
            task_control: XcpTaskControl::new(),
            a2l_file: None,
            calibration_objects: Vec::new(),
            measurement_objects: Vec::new(),
        }
    }
}
