//----------------------------------------------------------------------------------------------
// Module xcp

use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering},
    Arc, Mutex, Once,
};

use crate::{cal, reg, xcplib};
use cal::*;
use reg::*;

//----------------------------------------------------------------------------------------------
// XCP log level

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum XcpLogLevel {
    Off = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl From<u8> for XcpLogLevel {
    fn from(item: u8) -> XcpLogLevel {
        match item {
            0 => XcpLogLevel::Off,
            1 => XcpLogLevel::Error,
            2 => XcpLogLevel::Warn,
            3 => XcpLogLevel::Info,
            4 => XcpLogLevel::Debug,
            5 => XcpLogLevel::Trace,
            _ => XcpLogLevel::Warn,
        }
    }
}
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

impl XcpLogLevel {
    pub fn to_log_level_filter(self) -> log::LevelFilter {
        match self {
            XcpLogLevel::Off => log::LevelFilter::Off,
            XcpLogLevel::Error => log::LevelFilter::Error,
            XcpLogLevel::Warn => log::LevelFilter::Warn,
            XcpLogLevel::Info => log::LevelFilter::Info,
            XcpLogLevel::Debug => log::LevelFilter::Debug,
            XcpLogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

//----------------------------------------------------------------------------------------------
// XcpEvent

// Event number mapping lookup table
// The mapping of event numbers is used to create deterministic A2L files, regardless of the order of event creation
// The remapping is done when the registry is finalized and the A2L is written
// # Safety
// Use of a mutable static is save, because mutation for remapping is done only once in a thread safe context
static mut XCP_EVENT_MAP: [u16; XcpEvent::XCP_MAX_EVENTS] = [0; XcpEvent::XCP_MAX_EVENTS];

/// Represents a measurement event  
/// Holds the u16 XCP event number used in the XCP protocol and A2L to identify an event
/// May have an index > 0 to identify multiple events with the same name in instanciated in different threads
#[derive(Debug, Clone, Copy)]
pub struct XcpEvent {
    num: u16,   // Number used in A2L and XCP protocol
    index: u16, // Instance index, 0 if single instance
}

impl XcpEvent {

    /// Maximum number of events
    pub const XCP_MAX_EVENTS: usize = 256;

    // Uninitialized event
    pub const UNDEFINED: XcpEvent = XcpEvent {
        num: 0xFFFF,
        index: 0,
    };

    /// Create a new XCP event
    pub fn new(num: u16, index: u16) -> XcpEvent {
        assert!((num as usize) < XcpEvent::XCP_MAX_EVENTS, "Maximum number of events exceeded");
        unsafe {
            XCP_EVENT_MAP[num as usize] = num;
        }
        XcpEvent { num, index }
    }

    /// Get the event name
    pub fn get_name(self) -> &'static str {
        Xcp::get()
            .event_list
            .lock()
            .unwrap()
            .get_name(self)
            .unwrap()
    }

    // Get the event name with its index appended
    pub fn get_indexed_name(self) -> String {
        let name = self.get_name();
        if self.get_index() > 0 {
            format!("{}_{}", name, self.get_index())
        } else {
            name.to_string()
        }
    }

    /// Get the event number as u16
    /// Event number is a unique number for each event
    pub fn get_num(self) -> u16 {
        unsafe { XCP_EVENT_MAP[self.num as usize] }
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
        let a2l_addr: u32 = (self.get_num() as u32) << 16 | (offset as u16 as u32);
        (a2l_ext, a2l_addr)
    }

    /// Get address extension and address for A2L generation for XCP_ADDR_EXT_ABS addressing mode
    /// Used by A2L writer
    pub fn get_abs_ext_addr(self, addr: u64) -> (u8, u32) {
        let a2l_ext = Xcp::XCP_ADDR_EXT_ABS;
        let a2l_addr = unsafe { xcplib::ApplXcpGetAddr(addr as  *const u8) };
        (a2l_ext, a2l_addr)
    }

    /// Trigger a XCP event and provide a base pointer for relative addressing mode (XCP_ADDR_EXT_DYN)
    /// Address of the associated measurement variable must be relative to base
    /// 
    /// # Safety
    /// This is a C ffi call, which gets a pointer to a daq capture buffer
    /// The provenance of the pointer (len, lifetime) is is guaranteed , it refers to self
    /// The buffer must match its registry description, to avoid corrupt data given to the XCP tool
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn trigger(self, base: *const u8, len: u32) -> u8 {
        // trace!(
        //     "Trigger event {} num={}, index={}, base=0x{:X}, len={}",
        //     self.get_name(),
        //     self.get_num(),
        //     self.get_index(),
        //     base as u64,
        //     len
        // );
        // @@@@ unsafe - C library call
        // @@@@ unsafe - Transfering a pointer and its valid memory range to XCPlite FFI
        unsafe {
            // Trigger event
            xcplib::XcpEventExt(self.get_num(), base, len)
        }
    }

    /// Trigger a XCP event for measurement objects in absolute addressing mode (XCP_ADDR_EXT_DYN)
    /// Address of the associated measurement variable must be relative to module load addr
    /// In 64 applications, this offset might overflow in the A2L description - this is checked wenn generating A2L
    /// 
    /// # Safety
    /// This is a C ffi call, which gets a pointer to a daq capture buffer
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn trigger_abs(self) {
        // trace!(
        //     "Trigger event {} num={}, index={}, len={}",
        //     self.get_name(),
        //     self.get_num(),
        //     self.get_index(),
        //     len
        // );
        // @@@@ unsafe - C library call
        unsafe {
            // Trigger event
            xcplib::XcpEvent(self.get_num());
        }
    }
}

impl PartialEq for XcpEvent {
    fn eq(&self, other: &Self) -> bool {
        self.num == other.num
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
        unsafe {
            for (i, n) in XCP_EVENT_MAP.iter_mut().enumerate() {
                *n = i as u16;
            }
        }

        EventList(Vec::new())
    }

    fn clear(&mut self) {
        unsafe {
            XCP_EVENT_MAP = [0; XcpEvent::XCP_MAX_EVENTS];
        }
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
        self.0.sort_by(|a, b| {
            if a.name == b.name {
                a.event.index.cmp(&b.event.index)
            } else {
                a.name.cmp(b.name)
            }
        });
    }

    fn register(&mut self) {
        // Check event list is in untransformed order
        // @@@@ Remove this
        for (i, e) in self.0.iter_mut().enumerate() {
            assert!(e.event.num == i as u16);
            assert!(e.event.get_num() == i as u16);
        }

        // Sort the event list by name and then instance index
        self.sort_by_name_and_index();

        // Remap the event numbers
        // Problem is, that the event numbers are not deterministic, they depend on order of creation
        // This is not a problem for the XCP client, but the A2L file might change unnessesarily on every start of the application
        for (i, e) in self.0.iter().enumerate() {
            unsafe {
                XCP_EVENT_MAP[e.event.num as usize] = i as u16; // New external event number is index pointer to sorted list
            }
        }
        trace!("Event map: {:?}", unsafe { XCP_EVENT_MAP });

        // Register all events
        let r = Xcp::get().get_registry();
        self.0
            .iter()
            .for_each(|e| r.lock().unwrap().add_event(e.event));
    }

    fn create_event(&mut self, name: &'static str, indexed: bool) -> XcpEvent {
        // Allocate a new, sequential event number
        let num = self.0.len();

        // In instance mode (daq_create_event_instance), check for other events in instance mode with duplicate name and create new instance index
        let index = if indexed {
            self.0
                .iter()
                .filter(|e| e.name == name && e.event.get_index() > 0)
                .count()
                + 1
        } else {
            0
        };

        // Create XcpEvent
        let event = XcpEvent::new(num as u16, index as u16);

        info!(
            "Create event {} num={}, index={}",
            name,
            event.get_num(),
            event.get_index()
        );

        // Add XcpEventInfo to event list
        self.0.push(XcpEventInfo { name, event });

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
    Ram = XCP_CAL_PAGE_RAM as isize,
    Flash = XCP_CAL_PAGE_FLASH as isize,
}

impl From<u8> for XcpCalPage {
    fn from(item: u8) -> Self {
        match item {
            XCP_CAL_PAGE_RAM => XcpCalPage::Ram,
            XCP_CAL_PAGE_FLASH => XcpCalPage::Flash,
            _ => panic!("Invalid page value"),
        }
    }
}

//------------------------------------------------------------------------------------------
// XcpTransportLayer

/// enum to specify the transport layer of the XCP server
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XcpTransportLayer {
    Udp = 0,
    Tcp = 1,
}

impl XcpTransportLayer {
    /// Get the protocol name of the transport layer
    pub fn protocol_name(&self) -> &'static str {
        match self {
            XcpTransportLayer::Tcp => "TCP",
            XcpTransportLayer::Udp => "UDP",
        }
    }
}

//------------------------------------------------------------------------------------------
// XcpBuilder

/// A builder pattern to initialize the singleton instance of the XCP server
#[derive(Debug)]
pub struct XcpBuilder {
    log_level: XcpLogLevel, // log level for the server
    name: &'static str,     // Registry name, file name for the registry A2L generator
    a2l_enable: bool,       // Enable A2L file generation from registry
    epk: &'static str,      // EPK string for A2L version check
}

impl XcpBuilder {
    /// Create a XcpBuilder
    pub fn new(name: &'static str) -> XcpBuilder {
        XcpBuilder {
            log_level: XcpLogLevel::Info,
            name,
            a2l_enable: true,
            epk: "EPK",
        }
    }

    /// Set log level
    pub fn set_log_level(mut self, log_level: XcpLogLevel) -> Self {
        self.log_level = log_level;
        self
    }

    /// Enable A2L file generation
    pub fn enable_a2l(mut self, a2l_enable: bool) -> Self {
        self.a2l_enable = a2l_enable;
        self
    }

    /// Set the EPK to enable the XCP tool to check the A2L file fits the code
    pub fn set_epk(mut self, epk: &'static str) -> Self {
        self.epk = epk;
        self
    }

    /// Start the XCP protocol layer (used for testing only)
    pub fn start_protocol_layer(self) -> Result<&'static Xcp, &'static str> {
        let xcp = Xcp::get();

        // Server parameters from XcpBuilder
        Xcp::set_server_log_level(self.log_level);
        xcp.set_epk(self.epk);

        // Registry parameters from XcpBuiler
        {
            let mut r = xcp.registry.lock().unwrap();
            r.set_name(self.name);
            r.set_epk(self.epk, Xcp::XCP_EPK_ADDR);
        }

        Ok(xcp)
    }

    /// Start the XCP on Ethernet Transport Layer
    /// segment_size must fit the maximum UDP MTU supported by the system
    pub fn start_server(
        self,
        tl: XcpTransportLayer,
        addr: [u8; 4],
        port: u16,
        segment_size: u16,
    ) -> Result<&'static Xcp, &'static str> {
        let xcp = Xcp::get();

        // Server parameters from XcpBuilder
        Xcp::set_server_log_level(self.log_level);
        xcp.set_epk(self.epk);

        // Registry parameters from XcpBuiler
        {
            let mut r = xcp.registry.lock().unwrap();
            r.set_name(self.name);
            r.set_tl_params(tl.protocol_name(), addr, port); // Transport layer parameters
            r.set_epk(self.epk, Xcp::XCP_EPK_ADDR); // EPK
        }

        // @@@@ unsafe - C library call
        unsafe {
            // Initialize the XCP Server and ETH transport layer
            if 0 == xcplib::XcpEthServerInit(
                addr.as_ptr(),
                port,
                if tl == XcpTransportLayer::Tcp { 1 } else { 0 },
                segment_size,
            ) {
                return Err("Error: XcpEthServerInit() failed");
            }
        }

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
}

lazy_static::lazy_static! {
    static ref XCP_SINGLETON: Xcp = Xcp::new();
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


    // new
    fn new() -> Xcp {
        // @@@@ unsafe - C library call
        unsafe {
            xcplib::XcpInit();

            // Register the callbacks for xcplib
            xcplib::ApplXcpRegisterCallbacks(
                Some(cb_connect),
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
        }
    }

    /// Get the Xcp singleton instance
    #[inline(always)]
    pub fn get() -> &'static Xcp {
        &XCP_SINGLETON
    }

    //------------------------------------------------------------------------------------------
    // Associated functions

    /// Set the log level for XCP protocol layer
    pub fn set_server_log_level(level: XcpLogLevel) {
        // @@@@ unsafe - C library call
        unsafe {
            xcplib::ApplXcpSetLogLevel(level as u8);
        }
    }

    /// Check if the XCP server is ok and running
    pub fn check_server() -> bool {
        // @@@@ unsafe - C library call
        unsafe {
            // Return server status
            0 != xcplib::XcpEthServerStatus()
        }
    }

    /// Stop the XCP server
    pub fn stop_server() {
        // @@@@ unsafe - C library call
        unsafe {
            xcplib::XcpEthServerShutdown();
        }
    }

    /// Print a formated text message to the XCP client tool console
    pub fn print(msg: &str) {
        let msg = std::ffi::CString::new(msg).unwrap();
        // @@@@ unsafe - C library call
        unsafe {
            xcplib::XcpPrint(msg.as_ptr());
        }
    }

    //------------------------------------------------------------------------------------------
    // Calibration segments

    /// Create a calibration segment
    /// # Panics
    /// Panics if the calibration segment name already exists
    pub fn create_calseg<T>(
        name: &'static str,
        default_page: &'static T,
        load_json: bool,
    ) -> CalSeg<T>
    where
        T: CalPageTrait,
    {
        let mut m = Xcp::get().calseg_list.lock().unwrap();
        m.create_calseg(name, default_page, load_json)
    }

    /// Get calibration segment index by name
    pub fn get_calseg_index(&self, name: &str) -> Option<usize> {
        let m = self.calseg_list.lock().unwrap();
        m.get_index(name)
    }

    pub fn get_calseg_name(&self, index: usize) -> &'static str {
        let m = self.calseg_list.lock().unwrap();
        m.get_name(index)
    }

    /// Get registry addr base for a CalSeg
    pub fn get_calseg_addr_base(calseg_index: usize) -> u32 {
        (((calseg_index as u32) + 1) | 0x8000) << 16 // Address format for calibration segment field is index | 0x8000 in high word, addr_ext is 0 (CANape does not support addr_ext in memory segments)
    }

    // Get (ext,addr) for A2L generation of calibration values in a CalSeg
    // The address is relative to the base pointer of the calibration segment
    // The address extension is set to XCP_ADDR_EXT_DYN
    pub fn get_calseg_ext_addr(calseg_name: &str, offset: u16) -> (u8, u32) {
        let addr_ext = Xcp::XCP_ADDR_EXT_APP;
        let calseg_index = Xcp::get().get_calseg_index(calseg_name).unwrap();
        let addr: u32 = offset as u32 + Xcp::get_calseg_addr_base(calseg_index);
        (addr_ext, addr)
    }
    //------------------------------------------------------------------------------------------
    // EPK

    // Set EPK
    fn set_epk(&self, epk: &'static str) {
        let mut m = self.epk.lock().unwrap();
        *m = epk;
    }

    //------------------------------------------------------------------------------------------
    // XCP events

    // Create daq event
    // index==0 event is owned by a static in a function  (macro daq_create_event)
    // index>0 event is hold in thread local memory, index is the thread instance count (macro daq_create_event_instance)
    pub fn create_event(&self, name: &'static str, indexed: bool) -> XcpEvent {
        self.event_list.lock().unwrap().create_event(name, indexed)
    }

    //------------------------------------------------------------------------------------------
    // Registry

    /// Write A2L
    /// A2l is normally automatically written on connect of the XCP client tool
    /// This function force the A2L to be written immediately
    pub fn write_a2l(&self) {
        // Do nothing, if the registry is already written, or does not exist
        if self.registry.lock().unwrap().get_name().is_none() {
            return;
        }

        // Register all calibration segments
        self.calseg_list.lock().unwrap().register();

        // Register all events
        self.event_list.lock().unwrap().register();

        // Write A2L file from registry
        let mut r = self.registry.lock().unwrap();
        if let Ok(_res) = r.write() {
            // A2L exists and is up to date on disk
            // Set the name of the A2L file in the XCPlite server to enable upload via XCP
            let name = std::ffi::CString::new(r.get_name().unwrap()).unwrap();
            // @@@@ unsafe - C library call
            unsafe {
                xcplib::ApplXcpSetA2lName(name.as_ptr());
            }
            std::mem::forget(name); // This memory is never dropped, it is moved to xcplib singleton

            // A2l is no longer needed yet, free memory
            // Another call to a2l_write will do nothing
            // All registrations from now on, will cause panic
            r.clear();
        }
    }

    /// Get a clone of the registry
    pub fn get_registry(&self) -> Arc<Mutex<Registry>> {
        Arc::clone(&self.registry)
    }

    //------------------------------------------------------------------------------------------
    // Calibration page switching

    /// Set the active calibration page for the ECU access (used for test only)
    // @@@@ ToDo: remove this pub
    pub fn set_ecu_cal_page(&self, page: XcpCalPage) {
        self.ecu_cal_page.store(page as u8, Ordering::Relaxed);
    }

    /// Set the active calibration page for the XCP access (used for test only)
    // @@@@ ToDo: remove this pub
    pub fn set_xcp_cal_page(&self, page: XcpCalPage) {
        self.xcp_cal_page.store(page as u8, Ordering::Relaxed);
    }

    /// Get the active calibration page for the ECU access
    #[inline(always)]
    pub fn get_ecu_cal_page(&self) -> XcpCalPage {
        if self.ecu_cal_page.load(Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            XcpCalPage::Ram
        } else {
            XcpCalPage::Flash
        }
    }

    /// Get the active calibration page for the XCP access
    pub fn get_xcp_cal_page(&self) -> XcpCalPage {
        if self.ecu_cal_page.load(Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            XcpCalPage::Ram
        } else {
            XcpCalPage::Flash
        }
    }

    //------------------------------------------------------------------------------------------
    // Freeze and Init

    /// Set calibration segment init request
    /// Called on init cal from XCP server
    pub fn set_init_request(&self) {
        let mut m = self.calseg_list.lock().unwrap();
        m.set_init_request();
    }

    /// Set calibration segment freeze request
    /// Called on freeze cal from XCP server
    pub fn set_freeze_request(&self) {
        let mut m = self.calseg_list.lock().unwrap();
        m.set_freeze_request();
    }
}

//--------------------------------------------------------------------------------------------------------------------------------------------------
// Callback entrypoints for XCPlite C library protocol layer
// on connect, page switch handling, init and freeze calibration segment, read and write memory

// XCP error codes for callbacks from XCPlite
const TRUE: u8 = 1;
const CRC_CMD_OK: u8 = 0;
const CRC_PAGE_MODE_NOT_VALID: u8 = 0x27;
//const CRC_SEGMENT_NOT_VALID: u8 = 0x28;
const CRC_ACCESS_DENIED: u8 = 0x24;

// Modes for page switching
// @@@@ Clarify: Individual segment switching is not supported yet
const CAL_PAGE_MODE_ECU: u8 = 0x01;
const CAL_PAGE_MODE_XCP: u8 = 0x02;
const CAL_PAGE_MODE_ALL: u8 = 0x80; // switch all segments simultaneously

#[no_mangle]
extern "C" fn cb_connect() -> u8 {
    trace!("cb_connect: generate and write Al2 file");
    let xcp = Xcp::get();
    xcp.write_a2l();
    TRUE
}

// Switching individual segments (CANape option CALPAGE_SINGLE_SEGMENT_SWITCHING) not supported, not needed and CANape is buggy
// Returns 0xFF on invalid mode, segment number is ignored, CAL_PAGE_MODE_ALL is ignored
#[no_mangle]
extern "C" fn cb_get_cal_page(segment: u8, mode: u8) -> u8 {
    debug!(
        "cb_get_cal_page: get cal page of segment {}, mode {:02X}",
        segment, mode
    );
    let page: u8;
    if (mode & CAL_PAGE_MODE_ECU) != 0 {
        page = Xcp::get().get_ecu_cal_page() as u8;
        debug!("cb_get_cal_page: ECU page = {:?}", XcpCalPage::from(page));
    } else if (mode & CAL_PAGE_MODE_XCP) != 0 {
        page = Xcp::get().get_xcp_cal_page() as u8;
        debug!("cb_get_cal_page: XCP page = {:?}", XcpCalPage::from(page));
    } else {
        return 0xFF; // Invalid page mode
    }
    page
}

#[no_mangle]
extern "C" fn cb_set_cal_page(segment: u8, page: u8, mode: u8) -> u8 {
    debug!(
        "cb_set_cal_page: set cal page to segment={}, page={:?}, mode={:02X}",
        segment,
        XcpCalPage::from(page),
        mode
    );
    if (mode & CAL_PAGE_MODE_ALL) == 0 {
        return CRC_PAGE_MODE_NOT_VALID; // Switching individual segments not supported yet
    }

    // Ignore segment number
    // if segment > 0 && segment < 0xFF {
    //     return CRC_SEGMENT_NOT_VALID; // Only one segment supported yet
    // }

    let xcp = Xcp::get();
    if (mode & CAL_PAGE_MODE_ECU) != 0 {
        xcp.set_ecu_cal_page(XcpCalPage::from(page));
    }
    if (mode & CAL_PAGE_MODE_XCP) != 0 {
        xcp.set_xcp_cal_page(XcpCalPage::from(page));
    }

    CRC_CMD_OK
}

#[no_mangle]
extern "C" fn cb_init_cal(_src_page: u8, _dst_page: u8) -> u8 {
    trace!("cb_init_cal");
    Xcp::get().set_init_request();
    CRC_CMD_OK
}

#[no_mangle]
extern "C" fn cb_freeze_cal() -> u8 {
    trace!("cb_freeze_cal");
    Xcp::get().set_freeze_request();
    CRC_CMD_OK
}

#[no_mangle]
extern "C" fn cb_read(addr: u32, len: u8, dst: *mut u8) -> u8 {
    trace!("cb_read: addr=0x{:08X}, len={}, dst={:?}", addr, len, dst);
    assert!((addr & 0x80000000) != 0, "cb_read: invalid address");
    assert!(len > 0, "cb_read: zero length");

    // Decode addr
    let index: u16 = (addr >> 16) as u16 & 0x7FFF;
    let offset: u16 = (addr & 0xFFFF) as u16;

    // EPK read
    // This might be more elegantlty solved with a EPK segment in the registry, but this is a simple solution
    // Otherwise we would have to introduce a read only CalSeg
    if index == 0 {
        let m = Xcp::get().epk.lock().unwrap();
        let epk = *m;
        let epk_len = epk.len();

        assert!(
            offset as usize + len as usize <= epk_len && epk_len <= 0xFF,
            "cb_read: EPK length error ! offset={} len={} epk_len={}",
            offset,
            len,
            epk_len
        );

        // @@@@ unsafe - writing to a pointer from XCPlite FFI to get EPK, pointer arithmetic
        unsafe {
            let src = epk.as_ptr().add(offset as usize);
            std::ptr::copy_nonoverlapping(src, dst, len as usize);
        }
        CRC_CMD_OK
    }
    // Calibration segment read
    else {
        let calseg_list = Xcp::get().calseg_list.lock().unwrap();
        if !calseg_list.read_from((index - 1) as usize, offset, len, dst) {
            CRC_ACCESS_DENIED
        } else {
            CRC_CMD_OK
        }
    }
}

#[no_mangle]
extern "C" fn cb_write(addr: u32, len: u8, src: *const u8, delay: u8) -> u8 {
    trace!(
        "cb_write: dst=0x{:08X}, len={}, src={:?}, delay={}",
        addr,
        len,
        src,
        delay
    );
    assert!(len > 0, "cb_write: zero length");

    // Decode addr
    assert!((addr & 0x80000000) != 0, "cb_write: invalid address");
    let index: u16 = (addr >> 16) as u16 & 0x7FFF;
    if index == 0 {
        return CRC_ACCESS_DENIED; // EPK is read only
    }
    let offset: u16 = (addr & 0xFFFF) as u16;

    // Write to calibration segment
    let calseg_list = Xcp::get().calseg_list.lock().unwrap();
    if !calseg_list.write_to((index - 1) as usize, offset, len, src, delay) {
        CRC_ACCESS_DENIED
    } else {
        CRC_CMD_OK
    }
}

#[no_mangle]
extern "C" fn cb_flush() -> u8 {
    trace!("cb_flush");
    Xcp::get().calseg_list.lock().unwrap().flush();
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
    pub fn test_setup(x: log::LevelFilter) {
        TEST_INIT.call_once(|| {
            env_logger::Builder::new().filter_level(x).init();
        });

        test_reinit();
    }

    // Reinit XCP singleton before the next test
    pub fn test_reinit() {
        let xcp = Xcp::get();
        Xcp::set_server_log_level(XcpLogLevel::Warn);
        xcp.set_ecu_cal_page(XcpCalPage::Ram);
        xcp.set_xcp_cal_page(XcpCalPage::Ram);
        let mut l = xcp.event_list.lock().unwrap();
        l.clear();
        let mut s = xcp.calseg_list.lock().unwrap();
        s.clear();
        let mut r = xcp.registry.lock().unwrap();
        r.clear();
        r.set_name("xcp_lite");
        r.set_epk("TEST_EPK", Xcp::XCP_EPK_ADDR);
    }

    // Direct calls to the XCP driver callbacks for init and freeze
    #[allow(dead_code)]
    pub fn test_freeze_cal() {
        cb_freeze_cal();
    }
    #[allow(dead_code)]
    pub fn test_init_cal() {
        cb_init_cal(1, 0);
    }
    // pub fn test_set_cal_page(page: u8) {
    //     cb_set_cal_page(0, page, CAL_PAGE_MODE_XCP);
    // }
    // pub fn test_get_ecu_cal_page() -> u8 {
    //     cb_get_cal_page(0, CAL_PAGE_MODE_ECU)
    // }
    // pub fn test_get_xcp_cal_page() -> u8 {
    //     cb_get_cal_page(0, CAL_PAGE_MODE_XCP)
    // }
}
