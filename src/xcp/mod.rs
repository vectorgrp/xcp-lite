//----------------------------------------------------------------------------------------------
// Module xcp

#![allow(unused_imports)]

use bitflags::bitflags;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use crate::registry::{self, McEvent};

//-----------------------------------------------------------------------------
// Submodules

// Submodule daq
pub mod daq;

// Submodule cal
mod cal;
pub use cal::CalCell;
pub use cal::CalPageTrait;
pub use cal::CalSeg;
pub use cal::CalSegList;

pub mod xcplib;

//-----------------------------------------------------------------------------
// XCP println macro

/// Print formatted text to CANape console
#[allow(unused_macros)]
#[macro_export]
macro_rules! xcp_println {
    ( $fmt:expr ) => {
        Xcp::get().print(&format!($fmt));
    };
    ( $fmt:expr, $( $arg:expr ),* ) => {
        Xcp::get().print(&format!($fmt, $( $arg ),*));
    };
}

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
// Session status

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
// static XCP_EVENT_MAP: OnceCell<[u16; XcpEvent::XCP_MAX_EVENTS]> = OnceCell::new();

/// Represents a measurement event  
/// Holds the raw u16 event number used in the XCP protocol and in A2L IF_DATA to identify an event
/// May have an index > 0 to express multiple events with the same name are instantiated in different thread local instances
#[derive(Debug, Clone, Copy)]
pub struct XcpEvent {
    id: u16,    // Number used in A2L and XCP protocol
    index: u16, // Instance index, 0 if single instance
}

impl XcpEvent {
    /// Maximum number of events
    pub const XCP_MAX_EVENTS: u16 = 1024;
    /// Maximum number of thread local event instances
    pub const XCP_MAX_EVENT_INSTS: u16 = 255;
    /// Undefined event id number
    pub const XCP_UNDEFINED_EVENT_ID: u16 = 0xFFFF;

    /// Uninitialized event
    pub const XCP_UNDEFINED_EVENT: XcpEvent = XcpEvent {
        id: XcpEvent::XCP_UNDEFINED_EVENT_ID,
        index: 0,
    };

    /// Create a new XCP event
    pub fn new(id: u16, index: u16) -> XcpEvent {
        assert!(id < XcpEvent::XCP_MAX_EVENTS, "Maximum number of events exceeded");
        XcpEvent { id, index }
    }

    /// Get the event name
    pub fn get_name(self) -> &'static str {
        XCP.event_list.lock().get_name(self).unwrap()
    }

    // Get the event id as u16
    // Event id is a unique number for each event
    // pub fn get_id(self) -> u16 {
    //     if let Some(event_map) = XCP_EVENT_MAP.get() {
    //         event_map[self.id as usize]
    //     } else {
    //         panic!("XCP event map not initialized");
    //         //self.id
    //     }
    // }

    pub fn get_id(self) -> u16 {
        self.id
    }

    /// Get the event id as u16
    /// Event id is used to identify instances of the same function that generated this event with the same name
    /// This id is attached to signal names from different instances of the same signal
    pub fn get_index(self) -> u16 {
        self.index
    }

    /// Trigger a XCP event and provide a base pointer for relative addressing mode (XCP_ADDR_EXT_DYN or XCP_ADDR_EXT_REL)
    /// McAddress of the associated measurement variables must be relative to base
    ///
    /// # Safety
    /// This is a C ffi call, which gets a pointer to a daq capture buffer
    /// The provenance of the pointer (len, lifetime) is is guaranteed , it refers to self
    /// The buffer must match its registry description, to avoid corrupt data given to the XCP tool
    //#[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub unsafe fn trigger_ext(self, base: *const u8) {
        // @@@@ UNSAFE - C library call and transferring a pointer and its valid memory range to XCPlite FFI

        unsafe { xcplib::XcpEventExt(self.get_id(), base) }
    }
}

impl PartialEq for XcpEvent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for XcpEvent {
    fn default() -> Self {
        XcpEvent::XCP_UNDEFINED_EVENT
    }
}

//----------------------------------------------------------------------------------------------
// EventList

struct XcpEventInfo {
    name: &'static str,
    event: XcpEvent,
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

    // Register all events in the list and create the event id transformation map
    fn register(&mut self) {
        // Sort the event list by name and then instance index
        self.sort_by_name_and_index();

        // Remap the event numbers
        // Problem is, that the event numbers are not deterministic, they depend on order of creation
        // This is not a problem for the XCP client, but the A2L file might change unnecessarily on every start of the application
        // let mut event_map: [u16; XcpEvent::XCP_MAX_EVENTS] = [0; XcpEvent::XCP_MAX_EVENTS];
        // for (i, e) in self.0.iter().enumerate() {
        //     event_map[e.event.id as usize] = i.try_into().unwrap();
        // }
        // XCP_EVENT_MAP.set(event_map).ok();
        // log::trace!("Event map: {:?}", XCP_EVENT_MAP.get().unwrap());

        // Register all events
        {
            let mut l = registry::get_lock();
            let r = l.as_mut().unwrap();
            self.0.iter().for_each(|e| {
                let _ = r.event_list.add_event(McEvent::new(e.name, e.event.index, e.event.id, 0));
                // @@@@ TODO Error handling needed
            });
        }
    }

    fn create_event_ext(&mut self, name: &'static str, indexed: bool) -> XcpEvent {
        // Allocate a new, sequential event id number
        let id: u16 = self.0.len() as u16;
        if id >= XcpEvent::XCP_MAX_EVENTS {
            log::error!("Maximum number of events exceeded");
            return XcpEvent::XCP_UNDEFINED_EVENT;
        }

        // In instance mode, check for other events in instance mode with duplicate name and create new instance index
        // otherwise check for unique event name
        let index: u16 = if indexed {
            (self.0.iter().filter(|e| e.name == name && e.event.get_index() > 0).count() + 1).try_into().unwrap()
        } else {
            if self.0.iter().filter(|e| e.name == name).count() > 0 {
                log::error!("Event name {} already exists", name);
                return XcpEvent::XCP_UNDEFINED_EVENT;
            }
            0
        };
        if index > XcpEvent::XCP_MAX_EVENT_INSTS {
            log::error!("Maximum number of event thread local instances exceeded");
            return XcpEvent::XCP_UNDEFINED_EVENT;
        }

        // Create XcpEvent
        let event = XcpEvent::new(id, index);
        log::debug!("Create event {} id={}, index={}", name, event.get_id(), event.get_index());

        // Add XcpEventInfo to event list
        self.0.push(XcpEventInfo { name, event });
        event
    }
}

//------------------------------------------------------------------------------------------
// XcpCalPage

// Calibration page type (RAM,FLASH) used by the FFI
const XCP_CAL_PAGE_RAM: u8 = 0;
const XCP_CAL_PAGE_FLASH: u8 = 1;

/// Calibration page
/// enum to specify the active calibration page (mutable by XCP ("Ram") or const default ("Flash")) of a calibration segment
#[doc(hidden)] // For integration test
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XcpCalPage {
    /// The mutable page
    Ram = XCP_CAL_PAGE_RAM as isize,
    /// The default page
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
// Xcp singleton

/// A singleton instance of Xcp holds all XCP server data and states  
/// The Xcp singleton is obtained with Xcp::get()
pub struct Xcp {
    registry_finalized: AtomicBool,
    event_list: Arc<Mutex<EventList>>,
    epk: Mutex<&'static str>,

    ecu_cal_page: AtomicU8,

    xcp_cal_page: AtomicU8,

    calseg_list: Arc<Mutex<CalSegList>>,
}

lazy_static! {
    static ref XCP: Xcp = Xcp::new();
}

impl Xcp {
    /// Addr of the EPK
    pub const XCP_EPK_ADDR: u32 = 0x80000000;

    // new
    // Lazy static initialization of the Xcp singleton
    fn new() -> Xcp {
        unsafe {
            // Initialize the XCP protocol layer
            // @@@@ UNSAFE - C library calls
            xcplib::XcpInit(true);

            // Register the callbacks from xcplib
            // @@@@ UNSAFE - C library calls
            xcplib::ApplXcpRegisterCallbacks(
                Some(cb_connect),
                Some(cb_prepare_daq),
                Some(cb_start_daq),
                Some(cb_stop_daq),
                // @@@@ TODO Implement cb_freeze_daq
                None,
                Some(cb_get_cal_page),
                Some(cb_set_cal_page),
                Some(cb_freeze_cal),
                Some(cb_init_cal),
                Some(cb_read),
                Some(cb_write),
                Some(cb_flush),
            );
        }

        // Initialize the registry
        registry::init();

        // Create the Xcp singleton
        Xcp {
            registry_finalized: AtomicBool::new(false),
            event_list: Arc::new(Mutex::new(EventList::new())),
            epk: Mutex::new(""),

            ecu_cal_page: AtomicU8::new(XcpCalPage::Ram as u8), // ECU page defaults on RAM

            xcp_cal_page: AtomicU8::new(XcpCalPage::Ram as u8), // XCP page defaults on RAM

            calseg_list: Arc::new(Mutex::new(CalSegList::new())),
        }
    }

    /// Get the Xcp singleton instance
    #[inline]
    pub fn get() -> &'static Xcp {
        // XCP will be initialized by lazy_static
        &XCP
    }

    /// Set the log level for XCP C library xcplib
    #[allow(clippy::unused_self)]
    pub fn set_log_level(&self, level: u8) -> &'static Xcp {
        unsafe {
            // @@@@ UNSAFE - C library call
            xcplib::XcpSetLogLevel(level);
        }

        &XCP
    }

    /// Set the project name (will be used as A2L file name and A2L project name)
    pub fn set_app_name(&self, app_name: &str) -> &'static Xcp {
        registry::get_lock().as_mut().unwrap().set_app_info(app_name.to_string(), "xcp-lite", 0);
        &XCP
    }

    /// Set software version (will be used as A2L EPK string and for EPK memory segment)
    pub fn set_app_revision(&self, app_revision: &'static str) -> &'static Xcp {
        assert!(app_revision.len() % 4 == 0); // @@@@ TODO check length of EPK string
        *(self.epk.lock()) = app_revision;
        registry::get_lock().as_mut().unwrap().set_app_version(app_revision, Xcp::XCP_EPK_ADDR);
        &XCP
    }

    /// Set registry mode (flat or with typedefs, prefix names with app name)
    pub fn set_registry_mode(&self, flatten_typedefs: bool, prefix_names: bool) -> &'static Xcp {
        registry::get_lock().as_mut().unwrap().set_flatten_typedefs(flatten_typedefs);
        registry::get_lock().as_mut().unwrap().set_prefix_names(prefix_names);
        &XCP
    }

    /// Print a formatted text message to the XCP client tool console
    #[allow(clippy::unused_self)]
    pub fn print(&self, msg: &str) {
        // @@@@ UNSAFE - C library call

        unsafe {
            let msg = std::ffi::CString::new(msg).unwrap();
            xcplib::XcpPrint(msg.as_ptr());
        }
    }

    //------------------------------------------------------------------------------------------
    // XCP on Ethernet Server

    /// Start the XCP server
    pub fn start_server<A>(&self, tl: XcpTransportLayer, addr: A, port: u16, queue_size: u32) -> Result<&'static Xcp, XcpError>
    where
        A: Into<std::net::Ipv4Addr>,
    {
        {
            let ipv4_addr: std::net::Ipv4Addr = addr.into();
            let _ = &XCP;

            // Initialize the XCP server and ETH transport layer in xcplib
            unsafe {
                // @@@@ UNSAFE - C library call
                if !xcplib::XcpEthServerInit(&ipv4_addr.octets() as *const u8, port, tl == XcpTransportLayer::Tcp, queue_size) {
                    return Err(XcpError::XcpLib("Error: XcpEthServerInit() failed"));
                }
            }

            // Register transport layer parameters and actual ip addr of the server to create XCP IF_DATA make the A2L plug&play
            // If bound to any, get the actual ip address
            let mut addr: [u8; 4] = ipv4_addr.octets();
            if addr == [0, 0, 0, 0] {
                unsafe {
                    // @@@@ UNSAFE - C library call
                    xcplib::XcpEthServerGetInfo(std::ptr::null_mut(), std::ptr::null_mut(), &mut addr[0] as *mut u8, std::ptr::null_mut());
                }
            }
            let mut reg = registry::get_lock();
            if let Some(reg) = reg.as_mut() {
                reg.set_xcp_params(tl.protocol_name(), addr.into(), port); // Transport layer parameters
            }
            Ok(&XCP)
        }
    }

    /// Check if the XCP server is ok and running
    #[allow(clippy::unused_self)]
    pub fn check_server(&self) -> bool {
        unsafe {
            // @@@@ UNSAFE - C library call
            xcplib::XcpEthServerStatus()
        }
    }

    /// Stop the XCP server
    #[allow(clippy::unused_self)]
    pub fn stop_server(&self) {
        // @@@@ UNSAFE - C library calls

        unsafe {
            xcplib::XcpSendTerminateSessionEvent(); // Send terminate session event, if the XCP client is still connected
            xcplib::XcpDisconnect();
            xcplib::XcpEthServerShutdown();
        }
    }

    /// Signal the client to disconnect
    #[allow(clippy::unused_self)]
    pub fn disconnect_client(&self) {
        // @@@@ UNSAFE - C library calls

        unsafe {
            xcplib::XcpSendTerminateSessionEvent(); // Send terminate session event, if the XCP client is connected
        }
    }

    //------------------------------------------------------------------------------------------
    // Calibration segments

    /// Create a calibration segment  
    /// # Panics  
    /// If the calibration segment name already exists  
    /// If the calibration page size exceeds 64k
    pub fn create_calseg<T>(&self, name: &'static str, default_page: &'static T) -> CalSeg<T>
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
    fn get_calseg_name(&self, index: usize) -> &'static str {
        self.calseg_list.lock().get_name(index)
    }

    //------------------------------------------------------------------------------------------
    // XCP events

    /// Create XCP event  
    /// index==0 single instance  
    /// index>0 multi instance (instance number is attached to name)  
    pub fn create_event_ext(&self, name: &'static str, indexed: bool) -> XcpEvent {
        let event = self.event_list.lock().create_event_ext(name, indexed);
        if event == XcpEvent::XCP_UNDEFINED_EVENT {
            panic!("Event name already exists or maximum number of events exceeded");
        }
        event
    }

    /// Create XCP event  
    /// Single instance  
    pub fn create_event(&self, name: &'static str) -> XcpEvent {
        let event = self.event_list.lock().create_event_ext(name, false);
        if event == XcpEvent::XCP_UNDEFINED_EVENT {
            panic!("Event name already exists or maximum number of events exceeded");
        }
        event
    }

    //------------------------------------------------------------------------------------------
    // Registry
    // A2L file generation and provision for XCP upload

    /// Finalize the registry and provide it to the client tool
    /// A2l is normally automatically finalized on connect of the XCP client tool  
    /// After this happens, creating of registry content, like events and data objects is not possible anymore
    pub fn finalize_registry(&self) -> Result<bool, XcpError> {
        // Once
        // Ignore further calls
        if self.registry_finalized.load(Ordering::Relaxed) {
            return Ok(false);
        }
        assert!(!registry::is_closed());

        // Register all calibration segments

        self.calseg_list.lock().register();

        // Register all events
        self.event_list.lock().register();

        // Sort typedef, measurement, axis and calibration list to get a deterministic order
        // Event and CalSeg lists stay in the order they were added
        registry::get_lock().as_mut().unwrap().typedef_list.sort_by_name();
        registry::get_lock().as_mut().unwrap().instance_list.sort_by_name_and_event();

        // Close the registry and move to immutable state
        registry::close();

        // Write A2L

        {
            // Write A2L file from registry
            let write_xcp_ifdata = {
                // Build filename
                let app_name = registry::get().get_app_name();
                assert!(app_name.len() != 0, "App name not set");
                let mut path = std::path::PathBuf::new();
                path.set_file_name(app_name);
                path.set_extension("a2l");

                // Write A2L file to disk, with typedefs or flatten and mangle
                // @@@@ TODO parameter
                #[cfg(test)]
                let check = true;
                #[cfg(not(test))]
                let check = false;

                registry::get().write_a2l(&path, check)?;
                registry::get().has_xcp_params()
            };

            // If XCP is enabled:
            // Set the file name (without extension, assumed to be in current dir) of the A2L file in xcplib server to enable upload via XCP
            let _ = write_xcp_ifdata;

            if write_xcp_ifdata {
                unsafe {
                    let reg = registry::get();

                    let name = std::ffi::CString::new(reg.get_app_name()).unwrap();
                    // @@@@ UNSAFE - C library call
                    xcplib::ApplXcpSetA2lName(name.as_ptr());

                    let epk = std::ffi::CString::new(reg.get_app_version()).unwrap();
                    // @@@@ UNSAFE - C library call
                    xcplib::XcpSetEpk(epk.as_ptr());
                }
            }
        }

        // Mark the registration process as finished, A2l has been written and is ready for upload by XCP
        self.registry_finalized.store(true, Ordering::Relaxed);

        Ok(true)
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

    pub fn get_epk(&self) -> &Mutex<&'static str> {
        &self.epk
    }

    //------------------------------------------------------------------------------------------
    // Clock
    // For demo purposes:
    // Get the XCP 64Bit clock
    // Maybe 1us or 1ns resolution, arb or ptp epoch depending on the setting in xcplib main_cfg.h

    pub fn get_clock(&self) -> u64 {
        unsafe {
            // @@@@ UNSAFE - C library call
            xcplib::ApplXcpGetClock64()
        }
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
// @@@@ TODO Individual segment switching is not supported yet
const CAL_PAGE_MODE_ECU: u8 = 0x01;
const CAL_PAGE_MODE_XCP: u8 = 0x02;
const CAL_PAGE_MODE_ALL: u8 = 0x80; // switch all segments simultaneously

#[unsafe(no_mangle)]
extern "C" fn cb_connect() -> bool {
    {
        log::trace!("cb_connect: generate and write Al2 file");
        if let Err(e) = XCP.finalize_registry() {
            log::error!("connect refused, A2L file write failed, {}", e);
            return false;
        }
        true
    }
}

#[unsafe(no_mangle)]
#[allow(unused_variables)]
extern "C" fn cb_prepare_daq() -> u8 {
    log::trace!("cb_prepare_daq");
    1
}

#[unsafe(no_mangle)]
#[allow(unused_variables)]
extern "C" fn cb_start_daq() -> u8 {
    log::trace!("cb_start_daq");

    1
}

#[unsafe(no_mangle)]
extern "C" fn cb_stop_daq() {
    log::trace!("cb_stop_daq");
}

// Switching individual segments (CANape option CALPAGE_SINGLE_SEGMENT_SWITCHING) not supported, not needed and CANape is buggy
// Returns 0xFF on invalid mode, segment number is ignored, CAL_PAGE_MODE_ALL is ignored

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
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

#[unsafe(no_mangle)]
extern "C" fn cb_init_cal(_src_page: u8, _dst_page: u8) -> u8 {
    log::trace!("cb_init_cal");
    XCP.set_init_request();
    CRC_CMD_OK
}

#[unsafe(no_mangle)]
extern "C" fn cb_freeze_cal() -> u8 {
    log::trace!("cb_freeze_cal");
    XCP.set_freeze_request();
    CRC_CMD_OK
}

// Direct calibration memory access, read and write memory
// Here is the fundamental point of unsafety in XCP calibration
// Read and write are called by XCP on UPLOAD and DNLOAD commands and XCP must assure the correctness of the parameters, which are usually taken from an A2L file
// Writing with incorrect offset or len might lead to undefined behavior or at least wrong field values in the calibration segment
// Reading with incorrect offset or len will lead to incorrect data shown in the XCP tool
// @@@@ UNSAFE - direct memory access with pointer arithmetic

#[unsafe(no_mangle)]
unsafe extern "C" fn cb_read(addr: u32, len: u8, dst: *mut u8) -> u8 {
    log::trace!("cb_read: addr=0x{:08X}, len={}, dst={:?}", addr, len, dst);
    assert!((addr & 0x80000000) != 0, "cb_read: invalid address");
    assert!(len > 0, "cb_read: zero length");

    // Decode addr
    let index: u16 = (addr >> 16) as u16 & 0x7FFF;
    let offset: u16 = (addr & 0xFFFF) as u16;

    // EPK read
    // This might be more elegantly solved with a EPK segment in the registry, but this is a simple solution
    // Otherwise we would have to introduce a read only CalSeg
    if index == 0 {
        let m = XCP.epk.lock();
        let epk = *m;
        let epk_len = epk.len();

        // @@@@ TODO callbacks should not panic
        assert!(
            offset as usize + len as usize <= epk_len && epk_len <= 0xFF,
            "cb_read: EPK length error ! offset={} len={} epk_len={}",
            offset,
            len,
            epk_len
        );

        unsafe {
            let src = epk.as_ptr().add(offset as usize);
            std::ptr::copy_nonoverlapping(src, dst, len as usize);
        }
        CRC_CMD_OK
    }
    // Calibration segment read
    // read_from is Unsafe function
    else if !unsafe { XCP.calseg_list.lock().read_from((index - 1) as usize, offset, len, dst) } {
        CRC_ACCESS_DENIED
    } else {
        CRC_CMD_OK
    }
}

// @@@@ UNSAFE - direct memory access with pointer arithmetic

#[unsafe(no_mangle)]
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
    if !unsafe { XCP.calseg_list.lock().write_to((index - 1) as usize, offset, len, src, delay) } {
        CRC_ACCESS_DENIED
    } else {
        CRC_CMD_OK
    }
}

#[unsafe(no_mangle)]
extern "C" fn cb_flush() -> u8 {
    log::trace!("cb_flush");
    XCP.calseg_list.lock().flush();
    CRC_CMD_OK
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
pub mod xcp_test {
    use super::*;
    use std::sync::Once;

    // Using log level Info for tests reduces the probability of finding threading issues !!!
    static TEST_INIT: Once = Once::new();

    // Setup the test environment
    #[doc(hidden)]
    pub fn test_setup() -> &'static Xcp {
        // Log levels for tests
        const TEST_LOG_LEVEL: log::LevelFilter = log::LevelFilter::Info;
        const TEST_XCP_LOG_LEVEL: u8 = 2;

        // Initialize the logging subscriber once
        TEST_INIT.call_once(|| {
            env_logger::Builder::new()
                .target(env_logger::Target::Stdout)
                .filter_level(TEST_LOG_LEVEL)
                .format_timestamp(None)
                .format_module_path(false)
                .format_target(false)
                .init();
        });

        // Reinitialize the registry singleton
        registry::registry_test::test_reinit();

        // Reinitialize the XCP singleton
        let xcp = &XCP;
        xcp.set_log_level(TEST_XCP_LOG_LEVEL);
        xcp.event_list.lock().clear();

        {
            xcp.calseg_list.lock().clear();
            xcp.set_ecu_cal_page(XcpCalPage::Ram);
            xcp.set_xcp_cal_page(XcpCalPage::Ram);
        }

        xcp
    }
}
