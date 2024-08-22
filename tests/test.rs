// test
// Integration test for XCP basis functionality

// The tests are run in parallel by default, so the XCP instance is shared between tests
// The calibration segment list has to initialized bevore the test
// This means the tests can not run in parallel
// cargo test -- --test-threads=1
#![allow(unused_imports)]
#![allow(dead_code)]

use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Debug,
    sync::{Arc, Mutex, Once},
    thread,
    time::Instant,
};
use xcp::*;

//-----------------------------------------------------------------------------
// Extra bindings for testing

extern "C" {
    pub fn XcpTlInit(segmentSize: u16, cb: ::std::option::Option<unsafe extern "C" fn(msgLen: u16, msgBuf: *const u8) -> ::std::os::raw::c_int>) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn XcpTlShutdown();
}
extern "C" {
    pub fn XcpTlCommand(msgLen: u16, msgBuf: *const u8) -> u8;
}
extern "C" {
    pub fn XcpInit();
}
extern "C" {
    pub fn XcpStart();
}
extern "C" {
    pub fn XcpDisconnect();
}
// extern "C" {
//     pub fn XcpEvent(event: u16);
// }
extern "C" {
    pub fn XcpEventExt(event: u16, base: *const u8) -> u8;
}
extern "C" {
    pub fn XcpPrint(str_: *const ::std::os::raw::c_char);
}
extern "C" {
    pub fn XcpIsStarted() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn XcpIsConnected() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn XcpGetSessionStatus() -> u16;
}
extern "C" {
    pub fn XcpIsDaqRunning() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn ApplXcpGetPointer(xcpAddrExt: u8, xcpAddr: u32) -> *mut u8;
}
extern "C" {
    pub fn ApplXcpGetAddr(p: *const u8) -> u32;
}
extern "C" {
    pub fn XcpEthServerInit(addr: *const u8, port: u16, useTCP: ::std::os::raw::c_int, segmentSize: u16) -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn XcpEthServerShutdown() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn XcpEthServerStatus() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn set_log_level(level: u8);
}
extern "C" {
    pub fn ApplXcpSetA2lName(name: *const ::std::os::raw::c_char);
}
extern "C" {
    pub fn register_callback_connect(cb: ::std::option::Option<unsafe extern "C" fn() -> u8>);
}
extern "C" {
    pub fn register_callback_set_cal_page(cb: ::std::option::Option<unsafe extern "C" fn(segment: u8, page: u8, mode: u8) -> u8>);
}
extern "C" {
    pub fn register_callback_get_cal_page(cb: ::std::option::Option<unsafe extern "C" fn(segment: u8, mode: u8) -> u8>);
}
extern "C" {
    pub fn register_callback_freeze_cal(cb: ::std::option::Option<unsafe extern "C" fn() -> u8>);
}
extern "C" {
    pub fn register_callback_init_cal(cb: ::std::option::Option<unsafe extern "C" fn(src_page: u8, dst_page: u8) -> u8>);
}
extern "C" {
    pub fn socketStartup() -> ::std::os::raw::c_int;
}
extern "C" {
    pub fn socketCleanup();
}
extern "C" {
    pub fn clockInit() -> ::std::os::raw::c_int;
}

//-----------------------------------------------------------------------------

/// Used by the capture macros
/// Address transformation from A2L/XCP ext:u8/addr:u32 <-> Rust byte pointer *mut u8
pub fn xcp_get_pointer(addr_ext: u8, addr: u32) -> *mut u8 {
    // @@@@ unsafe - C library call
    unsafe { ApplXcpGetPointer(addr_ext, addr) }
}

/// Used by the capture macros
/// Address transformation from Rust byte pointer *const u8 <-> A2L/XCP addr:u32
pub fn xcp_get_abs_addr<T>(a: &T) -> u32 {
    // @@@@ unsafe - C library call
    unsafe { ApplXcpGetAddr(a as *const _ as *const u8) }
}

/// Get address extension used for absolute addressing
// pub fn xcp_get_abs_addr_ext() -> u8 {
//     XCP_ADDR_EXT_ABS
// }

/// XCP status
pub fn get_session_status() -> u16 {
    // @@@@ unsafe - C library call
    unsafe { XcpGetSessionStatus() }
}

/// XCP status
pub fn is_started() -> bool {
    // @@@@ unsafe - C library call
    unsafe { 0 != XcpIsStarted() }
}

/// XCP status
pub fn is_connected() -> bool {
    // @@@@ unsafe - C library call
    unsafe { 0 != XcpIsConnected() }
}

/// XCP status
pub fn is_daq_running() -> bool {
    // @@@@ unsafe - C library call
    unsafe { 0 != XcpIsDaqRunning() }
}

//-----------------------------------------------------------------------------

static TEST_INIT: Once = Once::new();

pub fn test_setup(level: XcpLogLevel) {
    // Using log level Info for tests reduces the probability of finding threading issues !!!
    TEST_INIT.call_once(|| {
        env_logger::Builder::new().filter_level(level.to_log_level_filter()).init();
    });

    // Reinitialize the XCP driver
    test_reinit();

    // Initialize the XCP driver transport layer only, not the server
    let _xcp = XcpBuilder::new("xcp_lite")
        .set_log_level(XcpLogLevel::Error)
        .enable_a2l(true)
        .set_epk("TEST_EPK")
        .start_protocol_layer()
        .unwrap();
}

//-----------------------------------------------------------------------------
// Test calibration page

use xcp_type_description::prelude::*;

// Calibration page struct
#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
struct CalPage {
    test_u8: u8,
    test_u64: u64,
    test_array: [[u8; 16]; 16],
}

// Calibration page default data
const CAL_PAR: CalPage = CalPage {
    test_u8: 0xAA,
    test_u64: 0x0000000100000002,
    test_array: [
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
        [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F],
    ],
};

//-----------------------------------------------------------------------------
// Generate transport layer test messages

#[repr(packed(1))]
#[allow(dead_code)]
struct XcpTlHeader {
    len: u16,
    ctr: u16,
}

// CONNECT
pub fn xcp_connect() {
    #[repr(packed(1))]
    #[allow(dead_code)]
    struct XcpCmdConnect {
        hdr: XcpTlHeader,
        cmd: u8,
        mode: u8,
    }
    let xcp_connect = XcpCmdConnect {
        hdr: XcpTlHeader { len: 2, ctr: 0 },
        cmd: 0xFF,
        mode: 0x00,
    };
    assert!(std::mem::size_of::<XcpCmdConnect>() == 6);
    // @@@@ uns afe - Intregration test
    unsafe {
        XcpTlInit(256, Some(cb_transmit));
        XcpStart();
        XcpTlCommand(6, &xcp_connect as *const XcpCmdConnect as *const u8);
        assert!(0 != XcpIsConnected());
    }
}

// SHORT_DOWNLOAD
pub fn xcp_short_download(addr: u32, ext: u8, len: u8, data_bytes: &[u8]) -> u8 {
    #[repr(packed(1))]
    #[allow(dead_code)]
    struct XcpCmdShortDownload {
        hdr: XcpTlHeader,
        cmd: u8,
        len: u8,
        res: u8,
        ext: u8,
        addr: u32,
        data: [u8; 256],
    }

    let mut xcp_short_download = XcpCmdShortDownload {
        hdr: XcpTlHeader { len: (8 + len) as u16, ctr: 0 },
        cmd: 0xED,
        len,
        res: 0,
        ext,
        addr,
        data: [0; 256],
    };
    xcp_short_download.data[..(len as usize)].copy_from_slice(&data_bytes[..(len as usize)]);
    // @@@@ unsafe - C library call
    unsafe { XcpTlCommand(4 + 8 + (len as u16), &xcp_short_download as *const XcpCmdShortDownload as *const u8) }
}

// DISCONNECT
pub fn xcp_disconnect() {
    #[repr(packed(1))]
    #[allow(dead_code)]
    struct XcpCmdDisconnect {
        hdr: XcpTlHeader,
        cmd: u8,
    }
    let xcp_disconnect = XcpCmdDisconnect {
        hdr: XcpTlHeader { len: 1, ctr: 0 },
        cmd: 0xFE,
    };
    // @@@@ unsafe - C library call
    unsafe {
        XcpTlCommand(5, &xcp_disconnect as *const XcpCmdDisconnect as *const u8);
        XcpTlShutdown();
    }
}

// XCP driver test command handler callback
#[no_mangle]
extern "C" fn cb_transmit(_msg_len: u16, _msg_buf: *const u8) -> libc::c_int {
    1 // ok
}

fn modify_test_u8(calseg: &CalSeg<CalPage>, value: u8) {
    let (ext, addr) = Xcp::get_calseg_ext_addr(
        calseg.get_name(),
        ((&CAL_PAR.test_u8 as *const u8 as usize) - (&CAL_PAR as *const CalPage as *const u8 as usize)) as u16,
    );
    let err = xcp_short_download(addr, ext, 1, &[value]);
    assert_eq!(err, 0);
    calseg.sync();
    assert_eq!(value, calseg.test_u8);
}

//-----------------------------------------------------------------------------
// Test platform

#[test]
fn test_platform() {
    test_setup(XcpLogLevel::Info);

    info!("Running test");
    if cfg!(target_endian = "little") {
        info!("The system is little endian! (Intel)");
    } else {
        error!("The system is big endian! (Motorola)");
        panic!("Big endian is not supported!");
    }
    info!("The system usize has {} bytes", std::mem::size_of::<usize>());
    info!("The system bool has {} bytes", std::mem::size_of::<bool>());
}

//-----------------------------------------------------------------------------
// Test CalSeg page switch via XCP
#[test]
fn test_calibration_segment_download() {
    test_setup(XcpLogLevel::Info);

    let cal_seg = Xcp::create_calseg("cal_seg", &CAL_PAR, false);
    Xcp::get().set_ecu_cal_page(XcpCalPage::Ram);
    Xcp::get().set_xcp_cal_page(XcpCalPage::Ram);
    xcp_connect();
    assert!(0xAA == cal_seg.test_u8);
    modify_test_u8(&cal_seg, 0x55);
    assert!(0x55 == cal_seg.test_u8);
    xcp_disconnect();
}

//-----------------------------------------------------------------------------
// Test performance

#[test]
fn test_cal_seg_sync_and_deref_performance() {
    test_setup(XcpLogLevel::Info);
    Xcp::get();
    let cal_seg = Xcp::create_calseg("cal_seg", &CAL_PAR, false);
    xcp_connect(); // write A2L
    modify_test_u8(&cal_seg, 1);
    assert_eq!(1, cal_seg.test_u8);
    let start_time = Instant::now();
    for _ in 0..10000 {
        if cal_seg.test_u8 != 1 {
            unreachable!();
        };
        cal_seg.sync();
    }
    let dt = start_time.elapsed().as_micros() as f64 / 10000.0;
    println!("CalSeg performance test: {} us per loop", dt);
    assert!(dt < 3.0, "CalSeg::sync or deref is unusually slow"); // us

    xcp_disconnect();
}
