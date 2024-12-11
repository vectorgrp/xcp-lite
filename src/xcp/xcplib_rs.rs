use super::XcpTransportLayer;

pub fn init() {
    unimplemented!();
}

pub fn register_callbacks(
    _cb_connect: ::std::option::Option<unsafe extern "C" fn() -> u8>,
    _cb_prepare_daq: ::std::option::Option<unsafe extern "C" fn() -> u8>,
    _cb_start_daq: ::std::option::Option<unsafe extern "C" fn() -> u8>,
    _cb_stop_daq: ::std::option::Option<unsafe extern "C" fn()>,
    _cb_get_cal_page: ::std::option::Option<unsafe extern "C" fn(segment: u8, mode: u8) -> u8>,
    _cb_set_cal_page: ::std::option::Option<unsafe extern "C" fn(segment: u8, page: u8, mode: u8) -> u8>,
    _cb_freeze_cal: ::std::option::Option<unsafe extern "C" fn() -> u8>,
    _cb_init_cal: ::std::option::Option<unsafe extern "C" fn(src_page: u8, dst_page: u8) -> u8>,
    _cb_read: ::std::option::Option<unsafe extern "C" fn(src: u32, size: u8, dst: *mut u8) -> u8>,
    _cb_write: ::std::option::Option<unsafe extern "C" fn(dst: u32, size: u8, src: *const u8, delay: u8) -> u8>,
    _cb_flush: ::std::option::Option<unsafe extern "C" fn() -> u8>,
) {
    unimplemented!();
}

pub fn disconnect() {
    unimplemented!();
}

pub fn event(_event: u16) {
    unimplemented!();
}

pub fn event_ext(_event: u16, _base: *const u8) -> u8 {
    unimplemented!();
}

pub fn print(_text: &str) {
    unimplemented!();
}

pub fn server_init(_addr: std::net::Ipv4Addr, _port: u16, _tl: XcpTransportLayer) -> bool {
    unimplemented!();
}

pub fn server_shutdown() -> bool {
    unimplemented!();
}
pub fn server_status() -> bool {
    unimplemented!();
}
