//----------------------------------------------------------------------------------------------
// Module xcp

use parking_lot::Mutex;
use std::{
    net::Ipv4Addr,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

// Using sync version of OnceCell from once_cell crate for the static event remapping array
use once_cell::sync::OnceCell;

// Using lazy_static crate for the XCP singleton
use lazy_static::lazy_static;

// Using bitflags crate for the XCP session status flags
use bitflags::bitflags;

use crate::reg;
use reg::*;

//-----------------------------------------------------------------------------
// Submodules

// Submodule daq
pub mod daq;

// Submodule cal
pub mod cal;
use cal::cal_seg::{CalPageTrait, CalSeg};
use cal::CalSegList;

// Use XCPlite xcplib as XCP server
mod xcplib;

//----------------------------------------------------------------------------------------------
// XCP error

use thiserror::Error;

#[derive(Error, Debug)]
pub enum XcpError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("xcplib error: `{0}` ")]
    XcpLib(&'static str),

    #[error("unknown error")]
    Unknown,
}

//----------------------------------------------------------------------------------------------
// Session statuc

bitflags! {
    /// Represents a set of flags for the XCP session status
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct XcpSessionStatus: u16 {
        const SS_DAQ            = 0x0040; // DAQ running
        const SS_INITIALIZED    = 0x8000;
        const SS_STARTED        = 0x4000;
        const SS_CONNECTED      = 0x2000;
    }
}

//----------------------------------------------------------------------------------------------
// XcpEvent

// Statically allocate memory for remapping XCP event numbers
// The mapping of event numbers is used to create deterministic A2L files, regardless of the order of event creation
// The remapping cell is initialized when the registry is finalized and the A2L is written
static XCP_EVENT_MAP: OnceCell<[u16; XcpEvent::XCP_MAX_EVENTS]> = OnceCell::new();

/// Represents a measurement event  
/// Glue needed for the macros
/// Holds the raw u16 XCP event number used in the XCP protocol and in A2L IF_DATA to identify an event
/// May have an index > 0 to express multiple events with the same name are instanciated in different thread local instances
#[derive(Debug, Clone, Copy)]
pub struct XcpEvent {
    channel: u16, // Number used in A2L and XCP protocol
    index: u16,   // Instance index, 0 if single instance
}

impl XcpEvent {
    /// Maximum number of events
    pub const XCP_MAX_EVENTS: usize = 1024;
    /// Undefined event channel number
    pub const XCP_UNDEFINED_EVENT_CHANNEL: u16 = 0xFFFF;

    /// Uninitialized event
    pub const XCP_UNDEFINED_EVENT: XcpEvent = XcpEvent {
        channel: XcpEvent::XCP_UNDEFINED_EVENT_CHANNEL,
        index: 0,
    };

    /// Create a new XCP event
    pub fn new(channel: u16, index: u16) -> XcpEvent {
        assert!((channel as usize) < XcpEvent::XCP_MAX_EVENTS, "Maximum number of events exceeded");
        XcpEvent { channel, index }
    }

    /// Get the event name
    pub fn get_name(self) -> &'static str {
        XCP.event_list.lock().get_name(self).unwrap()
    }

    /// Get the event number as u16
    /// Event number is a unique number for each event
    pub fn get_channel(self) -> u16 {
        if let Some(event_map) = XCP_EVENT_MAP.get() {
            event_map[self.channel as usize]
        } else {
            self.channel
        }
    }

    /// Get the event id as u16
    /// Event id is used to identify instances of the same function that generated this event with the same name
    /// This id is attached to signal names from different instances of the same signal
    pub fn get_index(self) -> u16 {
        self.index
    }

    /// Get address extension and address for A2L generation for XCP_ADDR_EXT_DYN addressing mode
    /// Used by A2L writer
    pub fn get_dyn_ext_addr(self, offset: i16) -> (u8, u32) {
        let a2l_ext = Xcp::XCP_ADDR_EXT_DYN;
        #[allow(clippy::cast_sign_loss)]
        let a2l_addr: u32 = (self.get_channel() as u32) << 16 | (offset as u16 as u32);
        (a2l_ext, a2l_addr)
    }

    /// Trigger a XCP event and provide a base pointer for relative addressing mode (XCP_ADDR_EXT_DYN)
    /// Address of the associated measurement variables must be relative to base
    ///
    /// # Safety
    /// This is a C ffi call, which gets a pointer to a daq capture buffer
    /// The provenance of the pointer (len, lifetime) is is guaranteed , it refers to self
    /// The buffer must match its registry description, to avoid corrupt data given to the XCP tool
    //#[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub unsafe fn trigger_ext(self, base: *const u8) -> u8 {
        // @@@@ Unsafe - C library call and transfering a pointer and its valid memory range to XCPlite FFI
        #[cfg(not(feature = "xcp_appl"))]
        unsafe {
            xcplib::XcpEventExt(self.get_channel(), base)
        }
        #[cfg(feature = "xcp_appl")]
        unsafe {
            let daq_lists = XCP.daq_lists.load(Ordering::Relaxed);
            if !daq_lists.is_null() {
                // DAQ running
                xcplib::XcpTriggerDaqEventAt(daq_lists, self.get_channel(), base, 0);
            }
            0
        }
    }

    /// Trigger a XCP event for absolute addressing DAQ lists (XCP_ADDR_EXT_ABS)
    /// Address of the associated measurement variables must be absolute (relative to ApplXcpGetBaseAddr)
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn trigger_abs(self) {
        // @@@@ Unsafe - C library call and transfering a pointer and its valid memory range to XCPlite FFI
        #[cfg(not(feature = "xcp_appl"))]
        unsafe {
            xcplib::XcpEvent(self.get_channel());
        }
        #[cfg(feature = "xcp_appl")]
        unsafe {
            let daq_lists = XCP.daq_lists.load(Ordering::Relaxed);
            if !daq_lists.is_null() {
                // DAQ running
                xcplib::XcpTriggerDaqEventAt(daq_lists, self.get_channel(), std::ptr::null_mut(), 0);
            }
        }
    }
}

impl PartialEq for XcpEvent {
    fn eq(&self, other: &Self) -> bool {
        self.channel == other.channel
    }
}

//----------------------------------------------------------------------------------------------
// EventList

struct XcpEventInfo {
    name: &'static str,
    event: XcpEvent,
    cycle_time_ns: u32, // 0 -sporadic or unknown
}

struct EventList(Vec<XcpEventInfo>);

impl EventList {
    fn new() -> EventList {
        EventList(Vec::new())
    }

    fn clear(&mut self) {
        self.0.clear();
    }

    fn get_name(&self, event: XcpEvent) -> Option<&'static str> {
        for e in &self.0 {
            if e.event == event {
                return Some(e.name);
            }
        }
        None
    }

    fn sort_by_name_and_index(&mut self) {
        self.0.sort_by(|a, b| if a.name == b.name { a.event.index.cmp(&b.event.index) } else { a.name.cmp(b.name) });
    }

    fn register(&mut self) {
        // Sort the event list by name and then instance index
        self.sort_by_name_and_index();

        // Remap the event numbers
        // Problem is, that the event numbers are not deterministic, they depend on order of creation
        // This is not a problem for the XCP client, but the A2L file might change unnessesarily on every start of the application
        let mut event_map: [u16; XcpEvent::XCP_MAX_EVENTS] = [0; XcpEvent::XCP_MAX_EVENTS];
        for (i, e) in self.0.iter().enumerate() {
            event_map[e.event.channel as usize] = i.try_into().unwrap();
        }
        XCP_EVENT_MAP.set(event_map).ok();
        log::trace!("Event map: {:?}", XCP_EVENT_MAP.get().unwrap());

        // Register all events
        let r = XCP.get_registry();
        {
            let mut l = r.lock();
            self.0.iter().for_each(|e| l.add_event(e.name, e.event, e.cycle_time_ns));
        }
    }

    fn create_event_ext(&mut self, name: &'static str, indexed: bool, cycle_time_ns: u32) -> XcpEvent {
        // Allocate a new, sequential event channel number
        let channel: u16 = self.0.len().try_into().unwrap();

        // In instance mode, check for other events in instance mode with duplicate name and create new instance index
        // otherwise check for unique event name
        let index: u16 = if indexed {
            (self.0.iter().filter(|e| e.name == name && e.event.get_index() > 0).count() + 1).try_into().unwrap()
        } else {
            assert!(self.0.iter().filter(|e| e.name == name).count() == 0, "Event name already exists");
            0
        };

        // Create XcpEvent
        let event = XcpEvent::new(channel, index);

        log::debug!("Create event {} channel={}, index={}", name, event.get_channel(), event.get_index());

        // Add XcpEventInfo to event list
        self.0.push(XcpEventInfo { name, event, cycle_time_ns });

        event
    }
}

//------------------------------------------------------------------------------------------
// XcpCalPage

// Calibration page type (RAM,FLASH) used by the FFI
pub const XCP_CAL_PAGE_RAM: u8 = 0;
pub const XCP_CAL_PAGE_FLASH: u8 = 1;

/// Calibration page
/// enum to specify the active calibration page (mutable by XCP ("Ram") or const default ("Flash")) of a calibration segment
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XcpCalPage {
    /// The mutable page
    Ram = XCP_CAL_PAGE_RAM as isize,
    /// The deafult page
    Flash = XCP_CAL_PAGE_FLASH as isize,
}

impl From<u8> for XcpCalPage {
    fn from(item: u8) -> Self {
        match item {
            XCP_CAL_PAGE_RAM => XcpCalPage::Ram,
            _ => XcpCalPage::Flash,
        }
    }
}

//------------------------------------------------------------------------------------------
// XcpTransportLayer

/// enum to specify the transport layer of the XCP server
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XcpTransportLayer {
    /// UDP transport layer
    Udp = 0,
    /// TCP transport layer
    Tcp = 1,
}

impl XcpTransportLayer {
    /// Get the protocol name of the transport layer
    pub fn protocol_name(self) -> &'static str {
        match self {
            XcpTransportLayer::Tcp => "TCP",
            XcpTransportLayer::Udp => "UDP",
        }
    }
}

//------------------------------------------------------------------------------------------
// XcpBuilder

/// A builder to initialize the singleton instance of the XCP server
#[derive(Debug)]
pub struct XcpBuilder {
    log_level: u8,      // log level for the server
    name: &'static str, // Registry name, file name for the registry A2L generator
    epk: &'static str,  // EPK string for A2L version check
}

impl XcpBuilder {
    /// Create a XcpBuilder
    pub fn new(name: &'static str) -> XcpBuilder {
        XcpBuilder { log_level: 3, name, epk: "EPK" }
    }

    /// Set log level
    #[must_use]
    pub fn set_log_level(mut self, log_level: u8) -> Self {
        self.log_level = log_level;
        self
    }

    /// Set the EPK to enable the XCP tool to check the A2L file fits the code
    #[must_use]
    pub fn set_epk(mut self, epk: &'static str) -> Self {
        self.epk = epk;
        self
    }

    /// Start the XCP on Ethernet Server
    pub fn start_server<A>(self, tl: XcpTransportLayer, addr: A, port: u16, queue_size: u32) -> Result<&'static Xcp, XcpError>
    where
        A: Into<Ipv4Addr>,
    {
        let ipv4_addr: Ipv4Addr = addr.into();
        let xcp = &XCP;

        // xcplib server log level parameter
        xcp.set_log_level(self.log_level);

        // EPV parameter
        xcp.set_epk(self.epk);

        // Register name and epk
        {
            let mut r = xcp.registry.lock();
            r.set_name(self.name);
            r.set_epk(self.epk, Xcp::XCP_EPK_ADDR); // EPK
        }

        // Initialize the XCP Server and ETH transport layer
        unsafe {
            // @@@@ Unsafe - C library call
            if !xcplib::XcpEthServerInit(&ipv4_addr.octets() as *const u8, port, tl == XcpTransportLayer::Tcp, queue_size) {
                return Err(XcpError::XcpLib("Error: XcpEthServerInit() failed"));
            }
        }

        // Register transport layer parameters and actual ip addr of the server to make the A2L plug&play
        let mut r = xcp.registry.lock();
        // If bound to any, get the actual ip address
        let mut addr: [u8; 4] = ipv4_addr.octets();
        if addr == [0, 0, 0, 0] {
            unsafe {
                // @@@@ Unsafe - C library call
                xcplib::XcpEthTlGetInfo(std::ptr::null_mut(), std::ptr::null_mut(), &mut addr[0] as *mut u8, std::ptr::null_mut());
            }
        }
        r.set_tl_params(tl.protocol_name(), addr.into(), port); // Transport layer parameters

        Ok(xcp)
    }
}

//------------------------------------------------------------------------------------------
// Xcp singleton

/// A singleton instance of Xcp holds all XCP server data and states  
/// The Xcp singleton is obtained with Xcp::get()
pub struct Xcp {
    ecu_cal_page: AtomicU8,
    xcp_cal_page: AtomicU8,
    event_list: Arc<Mutex<EventList>>,
    registry: Arc<Mutex<Registry>>,
    calseg_list: Arc<Mutex<CalSegList>>,
    epk: Mutex<&'static str>,
    #[cfg(feature = "xcp_appl")]
    daq_lists: std::sync::atomic::AtomicPtr<xcplib::tXcpDaqLists>,
}

lazy_static! {
    static ref XCP: Xcp = Xcp::new();
}

impl Xcp {
    /// Absolute addressing mode of XCPlite
    pub const XCP_ADDR_EXT_ABS: u8 = 1; // Used for DAQ objects on heap (addr is relative to module load address)
    /// Relative addressing mode of XCPlite
    pub const XCP_ADDR_EXT_DYN: u8 = 2; // Used for DAQ objects on stack and capture DAQ ( event in addr high word, low word relative to base given to XcpEventExt )
    /// Segment relative addressing mode of XCPlite handled by applications read/write callbacks
    pub const XCP_ADDR_EXT_APP: u8 = 0; // Used for CAL objects (addr = index | 0x8000 in high word (CANape does not support addr_ext in memory segments))

    /// Addr of the EPK
    pub const XCP_EPK_ADDR: u32 = 0x80000000;

    /// Get address extension and address for A2L generation for XCP_ADDR_EXT_ABS addressing mode
    /// Used by A2L writer
    pub fn get_abs_ext_addr(addr: u64) -> (u8, u32) {
        let a2l_ext = Xcp::XCP_ADDR_EXT_ABS;
        // @@@@ Unsafe - C library call
        let a2l_addr = unsafe { xcplib::ApplXcpGetAddr(addr as *const u8) };
        (a2l_ext, a2l_addr)
    }

    // new
    // Lazy static initialization of the Xcp singleton
    fn new() -> Xcp {
        unsafe {
            // Initialize the XCP protocol layer
            // @@@@ Unsafe - C library calls
            xcplib::XcpInit(std::ptr::null_mut());

            // Register the callbacks from xcplib
            // @@@@ Unsafe - C library calls
            xcplib::ApplXcpRegisterCallbacks(
                Some(cb_connect),
                Some(cb_prepare_daq),
                Some(cb_start_daq),
                Some(cb_stop_daq),
                None, // @@@@
                Some(cb_get_cal_page),
                Some(cb_set_cal_page),
                Some(cb_freeze_cal),
                Some(cb_init_cal),
                Some(cb_read),
                Some(cb_write),
                Some(cb_flush),
            );
        }

        Xcp {
            ecu_cal_page: AtomicU8::new(XcpCalPage::Ram as u8), // ECU page defaults on RAM
            xcp_cal_page: AtomicU8::new(XcpCalPage::Ram as u8), // XCP page defaults on RAM
            event_list: Arc::new(Mutex::new(EventList::new())),
            registry: Arc::new(Mutex::new(Registry::new())),
            calseg_list: Arc::new(Mutex::new(CalSegList::new())),
            epk: Mutex::new("DEFAULT_EPK"),
            #[cfg(feature = "xcp_appl")]
            daq_lists: std::sync::atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Get the Xcp singleton instance
    #[inline]
    #[must_use]
    pub fn get() -> &'static Xcp {
        // XCP will be initialized by lazy_static
        &XCP
    }

    #[allow(clippy::unused_self)]
    /// Set the log level for XCP library
    pub fn set_log_level(&self, level: u8) {
        unsafe {
            // @@@@ Unsafe - C library call
            xcplib::ApplXcpSetLogLevel(level);
        }
    }

    /// Print a formated text message to the XCP client tool console
    #[allow(clippy::unused_self)]
    pub fn print(&self, msg: &str) {
        // @@@@ Unsafe - C library call
        unsafe {
            let msg = std::ffi::CString::new(msg).unwrap();
            xcplib::XcpPrint(msg.as_ptr());
        }
    }

    //------------------------------------------------------------------------------------------
    // Server

    /// Check if the XCP server is ok and running
    #[allow(clippy::unused_self)]
    pub fn check_server(&self) -> bool {
        unsafe {
            // @@@@ Unsafe - C library call
            xcplib::XcpEthServerStatus()
        }
    }

    /// Stop the XCP server
    #[allow(clippy::unused_self)]
    pub fn stop_server(&self) {
        // @@@@ Unsafe - C library calls
        unsafe {
            xcplib::XcpSendTerminateSessionEvent(); // Send terminate session event, if the XCP client is still connected
            xcplib::XcpDisconnect();
            xcplib::XcpEthServerShutdown();
        }
    }

    /// Signal the client to disconnect
    #[allow(clippy::unused_self)]
    pub fn disconnect_client(&self) {
        // @@@@ Unsafe - C library calls
        unsafe {
            xcplib::XcpSendTerminateSessionEvent(); // Send terminate session event, if the XCP client is connected
        }
    }

    //------------------------------------------------------------------------------------------
    // Calibration segments

    /// Create a calibration segment  
    /// # Panics  
    /// Panics if the calibration segment name already exists  
    /// Panics if the calibration page size exceeds 64k
    pub fn create_calseg<T>(&self, name: &'static str, default_page: &'static T) -> CalSeg<T>
    where
        T: CalPageTrait,
    {
        self.calseg_list.lock().create_calseg(name, default_page)
    }

    /// Create a calibration segment, don't register fields and don't load json  
    /// # Panics  
    /// Panics if the calibration segment name already exists  
    /// Panics if the calibration page size exceeds 64k
    pub fn add_calseg<T>(&self, name: &'static str, default_page: &'static T) -> CalSeg<T>
    where
        T: CalPageTrait,
    {
        self.calseg_list.lock().create_calseg(name, default_page)
    }

    /// Get calibration segment index by name
    pub fn get_calseg_index(&self, name: &str) -> Option<usize> {
        self.calseg_list.lock().get_index(name)
    }

    /// Get calibration segment name by index
    pub fn get_calseg_name(&self, index: usize) -> &'static str {
        self.calseg_list.lock().get_name(index)
    }

    /// Get A2L addr (ext,addr) of a CalSeg
    pub fn get_calseg_ext_addr_base(calseg_index: u16) -> (u8, u32) {
        // Address format for calibration segment field is index | 0x8000 in high word, addr_ext is 0 (CANape does not support addr_ext in memory segments)
        let addr_ext = Xcp::XCP_ADDR_EXT_APP;
        let addr = (((calseg_index as u32) + 1) | 0x8000) << 16;
        (addr_ext, addr)
    }

    /// Get A2L addr (ext,addr) for a calibration value field at offset in a CalSeg
    /// The address is relative to the base addr of the calibration segment
    pub fn get_calseg_ext_addr(calseg_index: u16, offset: u16) -> (u8, u32) {
        let (addr_ext, mut addr) = Xcp::get_calseg_ext_addr_base(calseg_index);
        addr += offset as u32;
        (addr_ext, addr)
    }

    //------------------------------------------------------------------------------------------
    // EPK

    // Set EPK
    fn set_epk(&self, epk: &'static str) {
        *self.epk.lock() = epk;
    }

    //------------------------------------------------------------------------------------------
    // XCP events

    /// Create XCP event  
    /// index==0 single instance  
    /// index>0 multi instance (instance number is attached to name)  
    pub fn create_event_ext(&self, name: &'static str, indexed: bool, cycle_time_ns: u32) -> XcpEvent {
        self.event_list.lock().create_event_ext(name, indexed, cycle_time_ns)
    }

    /// Create XCP event  
    /// Single instance  
    pub fn create_event(&self, name: &'static str) -> XcpEvent {
        self.event_list.lock().create_event_ext(name, false, 0)
    }

    //------------------------------------------------------------------------------------------
    // Registry

    /// Write A2L  
    /// A2l is normally automatically written on connect of the XCP client tool  
    /// This function is used to force the A2L to be written immediately  
    pub fn write_a2l(&self) -> Result<bool, XcpError> {
        // Do nothing, if the registry is already written, or does not exist
        if self.registry.lock().is_frozen() {
            return Ok(false);
        }

        // Register all calibration segments
        self.calseg_list.lock().register();

        // Register all events
        self.event_list.lock().register();

        {
            // Write A2L file from registry
            self.registry.lock().write_a2l()?;

            // A2L exists and is up to date on disk
            // Set the name of the A2L file in the XCPlite server to enable upload via XCP

            unsafe {
                let name = std::ffi::CString::new(self.registry.lock().get_name().unwrap()).unwrap();
                // @@@@ Unsafe - C library call
                xcplib::ApplXcpSetA2lName(name.as_ptr());
                std::mem::forget(name); // This memory is never dropped, it is moved to xcplib singleton

                let epk = std::ffi::CString::new(self.registry.lock().get_epk().unwrap()).unwrap();
                // @@@@ Unsafe - C library call
                xcplib::ApplXcpSetEpk(epk.as_ptr());
                std::mem::forget(epk); // This memory is never dropped, it is moved to xcplib singleton
            }

            // A2l is no longer needed yet, free memory
            // Another call to a2l_write will do nothing
            // All registrations from now on, will cause panic
            self.registry.lock().freeze();
        }

        Ok(true)
    }

    /// Get a clone of the registry
    pub fn get_registry(&self) -> Arc<Mutex<Registry>> {
        Arc::clone(&self.registry)
    }

    //------------------------------------------------------------------------------------------
    // Calibration page switching

    /// Set the active calibration page for the ECU access (used for test only)
    fn set_ecu_cal_page(&self, page: XcpCalPage) {
        self.ecu_cal_page.store(page as u8, Ordering::Relaxed);
    }

    /// Set the active calibration page for the XCP access (used for test only)
    fn set_xcp_cal_page(&self, page: XcpCalPage) {
        self.xcp_cal_page.store(page as u8, Ordering::Relaxed);
    }

    /// Get the active calibration page for the ECU access
    #[inline]
    fn get_ecu_cal_page(&self) -> XcpCalPage {
        if self.ecu_cal_page.load(Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            XcpCalPage::Ram
        } else {
            XcpCalPage::Flash
        }
    }

    /// Get the active calibration page for the XCP access
    fn get_xcp_cal_page(&self) -> XcpCalPage {
        if self.xcp_cal_page.load(Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            XcpCalPage::Ram
        } else {
            XcpCalPage::Flash
        }
    }

    //------------------------------------------------------------------------------------------
    // Freeze and Init

    /// Set calibration segment init request  
    /// Called on init cal from XCP server  
    fn set_init_request(&self) {
        self.calseg_list.lock().set_init_request();
    }

    /// Set calibration segment freeze request  
    /// Called on freeze cal from XCP server  
    fn set_freeze_request(&self) {
        self.calseg_list.lock().set_freeze_request();
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Callback entrypoints for XCPlite C library protocol layer
// on connect, page switch handling, init and freeze calibration segment, read and write memory

// XCP error codes for callbacks from XCPlite
const FALSE: u8 = 0;
const TRUE: u8 = 1;
const CRC_CMD_OK: u8 = 0;
const CRC_MODE_NOT_VALID: u8 = 0x27;
//const CRC_SEGMENT_NOT_VALID: u8 = 0x28;
const CRC_ACCESS_DENIED: u8 = 0x24;

// Modes for page switching
// @@@@ Clarify: Individual segment switching is not supported yet
const CAL_PAGE_MODE_ECU: u8 = 0x01;
const CAL_PAGE_MODE_XCP: u8 = 0x02;
const CAL_PAGE_MODE_ALL: u8 = 0x80; // switch all segments simultaneously

#[no_mangle]
extern "C" fn cb_connect() -> u8 {
    log::trace!("cb_connect: generate and write Al2 file");
    if let Err(e) = XCP.write_a2l() {
        log::error!("connect refused, A2L file write failed, {}", e);
        return FALSE;
    }
    TRUE
}

#[no_mangle]
#[allow(unused_variables)]
extern "C" fn cb_prepare_daq(_daq: *const xcplib::tXcpDaqLists) -> u8 {
    log::trace!("cb_prepare_daq");
    TRUE
}

#[no_mangle]
#[allow(unused_variables)]
extern "C" fn cb_start_daq(daq: *const xcplib::tXcpDaqLists) -> u8 {
    log::trace!("cb_start_daq");
    #[cfg(feature = "xcp_appl")]
    XCP.daq_lists.store(daq.cast_mut(), Ordering::Relaxed);
    TRUE
}

#[no_mangle]
extern "C" fn cb_stop_daq() {
    log::trace!("cb_stop_daq");
    #[cfg(feature = "xcp_appl")]
    XCP.daq_lists.store(std::ptr::null_mut(), Ordering::Relaxed);
}

// Switching individual segments (CANape option CALPAGE_SINGLE_SEGMENT_SWITCHING) not supported, not needed and CANape is buggy
// Returns 0xFF on invalid mode, segment number is ignored, CAL_PAGE_MODE_ALL is ignored
#[no_mangle]
extern "C" fn cb_get_cal_page(segment: u8, mode: u8) -> u8 {
    log::debug!("cb_get_cal_page: get cal page of segment {}, mode {:02X}", segment, mode);
    let page: u8;
    if (mode & CAL_PAGE_MODE_ECU) != 0 {
        page = XCP.get_ecu_cal_page() as u8;
        log::debug!("cb_get_cal_page: ECU page = {:?}", XcpCalPage::from(page));
    } else if (mode & CAL_PAGE_MODE_XCP) != 0 {
        page = XCP.get_xcp_cal_page() as u8;
        log::debug!("cb_get_cal_page: XCP page = {:?}", XcpCalPage::from(page));
    } else {
        return 0xFF; // Invalid page mode
    }
    page
}

#[no_mangle]
extern "C" fn cb_set_cal_page(segment: u8, page: u8, mode: u8) -> u8 {
    log::debug!("cb_set_cal_page: set cal page to segment={}, page={:?}, mode={:02X}", segment, XcpCalPage::from(page), mode);
    if (mode & CAL_PAGE_MODE_ALL) == 0 {
        return CRC_MODE_NOT_VALID; // Switching individual segments not supported yet
    }

    // Ignore segment number
    // if segment > 0 && segment < 0xFF {
    //     return CRC_SEGMENT_NOT_VALID; // Only one segment supported yet
    // }

    if (mode & CAL_PAGE_MODE_ECU) != 0 {
        XCP.set_ecu_cal_page(XcpCalPage::from(page));
    }
    if (mode & CAL_PAGE_MODE_XCP) != 0 {
        XCP.set_xcp_cal_page(XcpCalPage::from(page));
    }

    CRC_CMD_OK
}

#[no_mangle]
extern "C" fn cb_init_cal(_src_page: u8, _dst_page: u8) -> u8 {
    log::trace!("cb_init_cal");
    XCP.set_init_request();
    CRC_CMD_OK
}

#[no_mangle]
extern "C" fn cb_freeze_cal() -> u8 {
    log::trace!("cb_freeze_cal");
    XCP.set_freeze_request();
    CRC_CMD_OK
}

// Direct calibration memory access, read and write memory
// Here is the fundamental point of unsafety in XCP calibration
// Read and write are called by XCP on UPLOAD and DNLOAD commands and XCP must assure the correctness of the parameters, which are usually taken from an A2L file
// Writing with incorrect offset or len might lead to undefined behaviour or at least wrong field values in the calibration segment
// Reading with incorrect offset or len will lead to incorrect data shown in the XCP tool
// @@@@ Unsafe - direct memory access with pointer arithmetic
#[no_mangle]
unsafe extern "C" fn cb_read(addr: u32, len: u8, dst: *mut u8) -> u8 {
    log::trace!("cb_read: addr=0x{:08X}, len={}, dst={:?}", addr, len, dst);
    assert!((addr & 0x80000000) != 0, "cb_read: invalid address");
    assert!(len > 0, "cb_read: zero length");

    // Decode addr
    let index: u16 = (addr >> 16) as u16 & 0x7FFF;
    let offset: u16 = (addr & 0xFFFF) as u16;

    // EPK read
    // This might be more elegantlty solved with a EPK segment in the registry, but this is a simple solution
    // Otherwise we would have to introduce a read only CalSeg
    if index == 0 {
        let m = XCP.epk.lock();
        let epk = *m;
        let epk_len = epk.len();

        // @@@@ callbacks should not panic
        assert!(
            offset as usize + len as usize <= epk_len && epk_len <= 0xFF,
            "cb_read: EPK length error ! offset={} len={} epk_len={}",
            offset,
            len,
            epk_len
        );

        let src = epk.as_ptr().add(offset as usize);
        std::ptr::copy_nonoverlapping(src, dst, len as usize);

        CRC_CMD_OK
    }
    // Calibration segment read
    // read_from is Unsafe function
    else if !XCP.calseg_list.lock().read_from((index - 1) as usize, offset, len, dst) {
        CRC_ACCESS_DENIED
    } else {
        CRC_CMD_OK
    }
}

// @@@@ Unsafe - direct memory access with pointer arithmetic
#[no_mangle]
unsafe extern "C" fn cb_write(addr: u32, len: u8, src: *const u8, delay: u8) -> u8 {
    log::trace!("cb_write: dst=0x{:08X}, len={}, src={:?}, delay={}", addr, len, src, delay);
    // @@@@ callbacks should not panic
    assert!(len > 0, "cb_write: zero length");

    // Decode addr
    assert!((addr & 0x80000000) != 0, "cb_write: invalid address");
    let index: u16 = (addr >> 16) as u16 & 0x7FFF;
    if index == 0 {
        return CRC_ACCESS_DENIED; // EPK is read only
    }
    let offset: u16 = (addr & 0xFFFF) as u16;

    // Write to calibration segment
    // read_from is Unsafe function
    if !XCP.calseg_list.lock().write_to((index - 1) as usize, offset, len, src, delay) {
        CRC_ACCESS_DENIED
    } else {
        CRC_CMD_OK
    }
}

#[no_mangle]
extern "C" fn cb_flush() -> u8 {
    log::trace!("cb_flush");
    XCP.calseg_list.lock().flush();
    CRC_CMD_OK
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Public test helpers

pub mod xcp_test {
    use super::*;
    use std::sync::Once;

    // Using log level Info for tests reduces the probability of finding threading issues !!!
    #[allow(dead_code)]
    static TEST_INIT: Once = Once::new();

    // Setup the test environment
    #[allow(dead_code)]
    pub fn test_setup(x: log::LevelFilter) -> &'static Xcp {
        log::info!("test_setup");
        TEST_INIT.call_once(|| {
            env_logger::Builder::new()
                .target(env_logger::Target::Stdout)
                .filter_level(x)
                .format_timestamp(None)
                .format_module_path(false)
                .format_target(false)
                .init();
        });

        test_reinit()
    }

    // Reinit XCP singleton before the next test
    pub fn test_reinit() -> &'static Xcp {
        let xcp = &XCP;
        xcp.set_log_level(2);
        {
            let mut l = xcp.event_list.lock();
            l.clear();
        }
        {
            let mut s = xcp.calseg_list.lock();
            s.clear();
        }
        {
            let mut r = xcp.registry.lock();
            r.clear();
            r.set_name("xcp_test");
            r.set_epk("TEST_EPK", Xcp::XCP_EPK_ADDR);
        }
        xcp.set_ecu_cal_page(XcpCalPage::Ram);
        xcp.set_xcp_cal_page(XcpCalPage::Ram);
        log::info!("Test reinit done");
        xcp
    }
}
