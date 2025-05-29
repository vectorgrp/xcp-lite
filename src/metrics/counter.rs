use crate::Xcp;
use crate::XcpEvent;
use crate::registry::*;

//-------------------------------------------------------------------------------------------------
// Macros

// Static state, thread local state not implemented yet
#[allow(unused_macros)]
#[macro_export]
macro_rules! metrics_counter {
    ( $name: expr ) => {{
        static __COUNTER_VALUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        static __COUNTER_EVENT: std::sync::OnceLock<XcpEvent> = std::sync::OnceLock::new();
        __COUNTER_VALUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let event = __COUNTER_EVENT.get_or_init(|| xcp_lite::metrics::counter::register($name));
        unsafe {
            event.trigger_ext(&__COUNTER_VALUE as *const _ as *const u8);
        }
    }};
}

/// Register the counter with the given name as measurement variable with dynamic addressing mode (async access (polling) possible)
/// # Panics
/// If the name is not unique in global measurement and calibration object name space
pub fn register(name: &'static str) -> XcpEvent {
    let event = Xcp::get().create_event_ext(name, false);
    let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
        name,
        McDimType::new(McValueType::Ulonglong, 1, 1, McSupportData::new(McObjectType::Measurement)),
        McAddress::new_event_dyn(event.get_id(), 0),
    );
    event
}
