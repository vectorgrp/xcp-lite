//----------------------------------------------------------------------------------------------
// Module xcp

#![allow(unused_imports)]

use bitflags::bitflags;
use lazy_static::lazy_static;
use parking_lot::Mutex;
use std::{
    ptr::null,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};
use xcp_type_description::XcpTypeDescription;

use crate::registry::{self, McEvent};

//-----------------------------------------------------------------------------
// Submodules

// Submodule daq
pub mod daq;

// Submodule cal
mod cal;
pub use cal::CalCell;
pub use cal::CalSeg;

// Submodule xcplib ffi c bindings
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
pub enum XcpClientError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("xcplib error: `{0}` ")]
    XcpLib(&'static str),

    #[error("unknown error")]
    Unknown,
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

    /// Get the event id as u16
    pub fn get_id(self) -> u16 {
        self.id
    }

    /// Get the event instance index as u16
    /// Event id is used to identify instances of the same function that generated this event with the same name
    /// This id is attached to signal names from different instances of the same signal
    pub fn get_index(self) -> u16 {
        self.index
    }

    /// Trigger a XCP event and provide a base pointer for relative addressing mode (XCP_ADDR_EXT_DYN)
    /// McAddress of the associated measurement variables must be relative to base
    ///
    /// # Safety
    /// This is a C ffi call, which gets a pointer to a daq capture buffer
    /// The provenance of the pointer (len, lifetime) is is guaranteed , it refers to self
    /// The buffer must match its registry description, to avoid corrupt data given to the XCP tool
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
}

lazy_static! {
    static ref XCP: Xcp = Xcp::new();
}

impl Xcp {
    fn new() -> Xcp {
        // Create the Xcp singleton
        Xcp {
            registry_finalized: AtomicBool::new(false),
            event_list: Arc::new(Mutex::new(EventList::new())),
        }
    }

    // Initialization of the Xcp singleton
    pub fn init(app_name: &str, app_revision: &str, log_level: u8) -> &'static Xcp {
        // Initialize the XCP library
        // @@@@ UNSAFE - C library calls
        unsafe {
            xcplib::XcpSetLogLevel(log_level);
            assert!(app_revision.len() < crate::EPK_SEG_SIZE);
            let epk = std::ffi::CString::new(app_revision).unwrap();
            assert!(app_revision.len() < crate::EPK_SEG_SIZE);
            let name = std::ffi::CString::new(app_name).unwrap();
            xcplib::XcpInit(name.as_ptr(), epk.as_ptr(), true);
            xcplib::ApplXcpRegisterConnectCallback(Some(cb_connect));
        }

        // Initialize the registry
        registry::init();
        registry::get_lock().as_mut().unwrap().application.set_info(app_name.to_string(), "xcp-lite", 0);
        registry::get_lock()
            .as_mut()
            .unwrap()
            .application
            .set_version(app_revision.to_string(), crate::EPK_SEG_ADDR);

        &XCP
    }

    /// Get the Xcp singleton instance
    #[inline]
    pub fn get() -> &'static Xcp {
        // XCP will be initialized by lazy_static
        &XCP
    }

    /// Set registry mode (flat or with typedefs, prefix names with app name)
    pub fn set_registry_mode(&self, flatten_typedefs: bool, prefix_names: bool) -> &'static Xcp {
        registry::get_lock().as_mut().unwrap().set_flatten_typedefs_mode(flatten_typedefs);
        registry::get_lock().as_mut().unwrap().set_prefix_names_mode(prefix_names);
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
    pub fn start_server<A>(&self, tl: XcpTransportLayer, addr: A, port: u16, queue_size: u32) -> Result<&'static Xcp, XcpClientError>
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
                    return Err(XcpClientError::XcpLib("Error: XcpEthServerInit() failed"));
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

    /// Get calibration segment index by name
    pub fn get_calseg_index(&self, name: &str) -> Option<usize> {
        unsafe {
            // @@@@ UNSAFE - C library call
            let c_name = std::ffi::CString::new(name).unwrap();
            let index = xcplib::XcpFindCalSeg(c_name.as_ptr());
            if index == u16::MAX {
                return None;
            }
            Some(index as usize)
        }
    }

    /// Get calibration segment name by index
    fn get_calseg_name(&self, index: usize) -> &'static str {
        unsafe {
            // @@@@ UNSAFE - C library call
            let name_ptr = xcplib::XcpGetCalSegName(index as u16);
            if !name_ptr.is_null() {
                let c_str = std::ffi::CStr::from_ptr(name_ptr);
                return c_str.to_str().unwrap_or("");
            } else {
                panic!("Calibration segment {} does not exist", index);
            }
        }
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
    pub fn finalize_registry(&self) -> Result<bool, XcpClientError> {
        // Once
        // Ignore further calls
        if self.registry_finalized.load(Ordering::Relaxed) {
            return Ok(false);
        }
        assert!(!registry::is_closed());

        // Register all calibration segments

        let calseg_count: u16 = unsafe { xcplib::XcpGetCalSegCount() };
        for i in 0..calseg_count {
            let name = unsafe { std::ffi::CStr::from_ptr(xcplib::XcpGetCalSegName(i)).to_str().unwrap() };
            let size = unsafe { xcplib::XcpGetCalSegSize(i) };
            log::info!("Register CalSeg {}, size={}", name, size);
            let _ = registry::get_lock().as_mut().unwrap().cal_seg_list.add_cal_seg(name, i, size as u32);
        }

        // Register all events
        self.event_list.lock().register();

        // Sort typedef, measurement, axis and calibration list to get a deterministic order
        // Event and CalSeg lists stay in the order they were added
        registry::get_lock().as_mut().unwrap().typedef_list.sort_by_name();
        registry::get_lock().as_mut().unwrap().instance_list.sort_by_name_and_event();

        // Close the registry and move to immutable state
        registry::close();

        // Write A2L file from registry
        // Build filename
        let app_name = registry::get().application.get_name();
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
        registry::get().write_a2l(&path, "xcp-lite", app_name, "", app_name, "XCPLITE__CASDD", check)?;

        // Notify xcplib of the A2L file
        unsafe {
            let reg = registry::get();
            let name = std::ffi::CString::new(reg.application.get_name()).unwrap();
            // @@@@ UNSAFE - C library call
            xcplib::XcpSetA2lName(name.as_ptr());
        }

        // Mark the registration process as finished, A2l has been written and is ready for upload by XCP
        self.registry_finalized.store(true, Ordering::Relaxed);

        Ok(true)
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

// on connect
#[unsafe(no_mangle)]
extern "C" fn cb_connect(_mode: u8) -> bool {
    {
        log::trace!("cb_connect: generate and write Al2 file");
        if let Err(e) = XCP.finalize_registry() {
            log::error!("connect refused, A2L file write failed, {}", e);
            return false;
        }
        true
    }
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
        unsafe {
            xcplib::XcpReset();
        }
        let xcp = Xcp::init("Test", "EPK_V1.1.0", TEST_XCP_LOG_LEVEL);
        xcp.event_list.lock().clear();

        xcp
    }
}
