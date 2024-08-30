//----------------------------------------------------------------------------------------------
// Module daq_event

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::{reg::RegistryMeasurement, xcp::*, RegistryDataType};

//----------------------------------------------------------------------------------------------
// XcpEvent

impl Xcp {
    // Create a measurement event and a measurement variable directly associated to the event with memory offset 0
    pub fn create_measurement_object(&self, name: &'static str, data_type: RegistryDataType, x_dim: u16, y_dim: u16, comment: &'static str) -> XcpEvent {
        let event = self.create_event(name);
        self.get_registry().lock().unwrap().add_measurement(RegistryMeasurement::new(
            name.to_string(),
            data_type,
            x_dim,
            y_dim,
            event,
            0, // byte_offset
            0,
            1.0, // factor
            0.0, // offset
            comment,
            "", // unit
            None,
        ));
        event
    }
}

/// Create a single instance XCP event and register the given variable once, trigger the event
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_event_ref {

    ( $id:expr, $data_type: expr, $x_dim: expr, $comment:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_measurement_object(stringify!($id), $data_type, $x_dim, 1, $comment);
        }
        XCP_EVENT__.trigger(&(*$id) as *const _ as *const u8, 0 );
    }};
    ( $id:expr, $data_type: expr, $x_dim: expr, $y_dim: expr, $comment:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_measurement_object(stringify!($id), $data_type, $x_dim, $y_dim, $comment);
        }
        XCP_EVENT__.trigger_ext(&(*$id) as *const _ as *const u8, 0);
    }};
}

//----------------------------------------------------------------------------------------------
// DaqEvent

/// DaqEvent is a wrapper for XcpEvent which adds on optional capture buffer (N may be 0)
#[derive(Debug)]
pub struct DaqEvent<const N: usize> {
    event: XcpEvent,
    buffer_len: usize,
    pub buffer: [u8; N],
}

impl<const N: usize> DaqEvent<N> {
    pub fn new(name: &'static str) -> DaqEvent<N> {
        let xcp = Xcp::get();
        DaqEvent {
            event: xcp.create_event_ext(name, false),
            buffer_len: 0,
            buffer: [0; N],
        }
    }
    pub fn new_from(xcp_event: &XcpEvent) -> DaqEvent<N> {
        DaqEvent {
            event: *xcp_event,
            buffer_len: 0,
            buffer: [0; N],
        }
    }

    fn get_xcp_event(&self) -> XcpEvent {
        self.event
    }

    /// Allocate space in the capture buffer
    pub fn allocate(&mut self, size: usize) -> i16 {
        trace!("Allocate DAQ buffer, size={}, len={}", size, self.buffer_len);
        let offset = self.buffer_len;
        assert!(offset + size <= self.buffer.len(), "DAQ buffer overflow");
        self.buffer_len += size;
        offset as i16
    }

    /// Copy to the capture buffer     
    pub fn capture(&mut self, data: &[u8], offset: i16) {
        self.buffer[offset as usize..offset as usize + data.len()].copy_from_slice(data);
    }

    /// Trigger for stack or capture buffer measurement with base pointer relative addressing
    pub fn trigger(&self) {
        let base: *const u8 = &self.buffer as *const u8;
        self.event.trigger_ext(base, self.buffer_len as u32);
    }

    /// Trigger for stack measurement with absolute addressing
    pub fn trigger_abs(&self) {
        self.event.trigger_abs();
    }

    /// Associate a variable to this DaqEvent, allocate space in the capture buffer and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_capture(
        &mut self,
        name: &'static str,
        size: usize,
        datatype: RegistryDataType,
        x_dim: u16,
        y_dim: u16,
        factor: f64,
        offset: f64,
        unit: &'static str,
        comment: &'static str,
        annotation: Option<String>,
    ) -> i16 {
        let event_offset: i16 = self.allocate(size); // Address offset (signed) relative to event memory context (XCP_ADDR_EXT_DYN)
        trace!("Allocate DAQ buffer for {}, TLS OFFSET = {} {:?} and register measurement", name, event_offset, datatype);
        let event = self.get_xcp_event();
        Xcp::get().get_registry().lock().unwrap().add_measurement(RegistryMeasurement::new(
            name.to_string(),
            datatype,
            x_dim,
            y_dim,
            event,
            event_offset,
            0u64,
            factor,
            offset,
            comment,
            unit,
            annotation,
        ));
        event_offset
    }

    /// Associate a variable on stack to this DaqEvent and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_stack(&self, name: &'static str, ptr: *const u8, datatype: RegistryDataType, x_dim: u16, y_dim: u16, factor: f64, offset: f64, unit: &'static str, comment: &'static str) {
        let p = ptr as usize; // variable address
        let b = &self.buffer as *const _ as usize; // base address
        debug!("add_stack: {} {:?} ptr={:p} base={:p}", name, datatype, ptr, &self.buffer as *const _);
        let o: i64 = p as i64 - b as i64; // variable - base address
        assert!((-0x8000..=0x7FFF).contains(&o), "memory offset out of range");
        let event_offset: i16 = o as i16;
        Xcp::get().get_registry().lock().unwrap().add_measurement(RegistryMeasurement::new(
            name.to_string(),
            datatype,
            x_dim,
            y_dim,
            self.event,
            event_offset,
            0u64,
            factor,
            offset,
            comment,
            unit,
            None,
        ));
    }

    /// Associate a variable on stack to this DaqEvent and register it
    #[allow(clippy::too_many_arguments)]
    pub fn add_heap(&self, name: &'static str, ptr: *const u8, datatype: RegistryDataType, x_dim: u16, y_dim: u16, factor: f64, offset: f64, unit: &'static str, comment: &'static str) {
        debug!("add_heap: {} {:?} ptr={:p} ", name, datatype, ptr,);

        Xcp::get().get_registry().lock().unwrap().add_measurement(RegistryMeasurement::new(
            name.to_string(),
            datatype,
            x_dim,
            y_dim,
            self.event,
            0i16,
            ptr as u64,
            factor,
            offset,
            comment,
            unit,
            None,
        ));
    }
}

//-----------------------------------------------------------------------------
// single instance (static) event
//-----------------------------------------------------------------------------

/// Create a static DAQ event or return the DAQ event if it already exists
/// This is a single static instance of a DAQ event
/// Even if the function is called multiple times, the DAQ event is created only once
/// This is thread safe
/// Multiple concurrently runing instances of a task use the same DAQ event
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_create_event {
    // Without capture buffer
    ( $name:expr, $capacity: expr ) => {{
        // Scope for lazy static XCP_EVENT__, create the XCP event only once
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_event($name);
        }
        // Create the DAQ event every time the thread is running through this code
        DaqEvent::<{ $capacity }>::new_from(&XCP_EVENT__)
    }};
    // With capture buffer capacity
    ( $name:expr ) => {{
        lazy_static::lazy_static! {
            static ref XCP_EVENT__: XcpEvent = Xcp::get().create_event($name);
        }
        DaqEvent::<0>::new_from(&XCP_EVENT__)
    }};
}

/// Capture the value of a variable with basic type for the given daq event
/// Register the given meta data once
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_capture {
    // name, event, comment, unit, factor,offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    $factor,
                    $offset,
                    $unit,
                    $comment,
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };

        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};

    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    1.0,
                    0.0,
                    $unit,
                    $comment,
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};

    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    $id.get_type(),
                    1, // x_dim
                    1, // y_dim
                    1.0,
                    0.0,
                    "",
                    "",
                    None,
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        $daq_event.capture(&($id.to_le_bytes()), byte_offset);
    }};
}

// @@@@ Experimental for point_cloud demo
// Capture the serialized value of an instance for the given daq event
// Register the given meta data and the serialization schema once
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_serialize {
    // name, event, comment
    ( $id:ident, $daq_event:expr, $comment:expr) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        let byte_offset;
        match DAQ_OFFSET__.compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed) {
            Ok(_) => {
                // @@@@ Experimental: Hard coded type here for point_cloud demo
                let annotation = GeneratorCollection::generate(&IDL::CDR, &$id.description()).unwrap();
                byte_offset = $daq_event.add_capture(
                    stringify!($id),
                    std::mem::size_of_val(&$id),
                    RegistryDataType::Blob,
                    $daq_event.buffer.len() as u16, // x_dim is buffer size in bytes
                    1,                              // y_dim
                    1.0,
                    0.0,
                    "",
                    $comment,
                    Some(annotation),
                );
                DAQ_OFFSET__.store(byte_offset, std::sync::atomic::Ordering::Relaxed);
            }
            Err(offset) => byte_offset = offset,
        };
        let v = cdr::serialize::<_, _, cdr::CdrBe>(&$id, cdr::Infinite).unwrap();
        $daq_event.capture(&v, byte_offset);
    }};
}

/// Register a local variable with basic type for the given daq event
/// Address will be relative to the stack frame position of event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register {
    // name, event, comment, unit, factor, offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        if DAQ_OFFSET__
            .compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, $factor, $offset, $unit, $comment);
        };
    }};
    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        if DAQ_OFFSET__
            .compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, $unit, $comment);
        };
    }};
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        if DAQ_OFFSET__
            .compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, "", "");
        };
    }};
}

/// Register a local variable which is a reference to heap with basic type for the given daq event
/// Address will be absolute addressing mode
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_ref {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        if DAQ_OFFSET__
            .compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            $daq_event.add_heap(stringify!($id), &(*$id) as *const _ as *const u8, (*$id).get_type(), 1, 1, 1.0, 0.0, "", "", None);
        };
    }};
}

/// Register a local variable with basic array type for the given daq event
/// Address will be relative to the stack frame position of event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_array {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        static DAQ_OFFSET__: std::sync::atomic::AtomicI16 = std::sync::atomic::AtomicI16::new(-32768);
        if DAQ_OFFSET__
            .compare_exchange(-32768, 0, std::sync::atomic::Ordering::Relaxed, std::sync::atomic::Ordering::Relaxed)
            .is_ok()
        {
            let dim = (std::mem::size_of_val(&$id) / std::mem::size_of_val(&$id[0])) as u16;
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, ($id[0]).get_type(), dim, 1, 1.0, 0.0, "", "");
        };
    }};
}

//-----------------------------------------------------------------------------
// multi instance (TLS) event
//-----------------------------------------------------------------------------

/// Create a multi instance task DAQ event or return the DAQ event if it already exists
/// The DAQ event lives in thread local storage (TLS)
/// When the macro is called multiple times, the DAQ event is created once for each thread
/// This is thread safe, there is no potential race with other threads
/// Multiple concurrently runing instances of a task use the DAQ event assiated to their thread
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_create_event_instance {
    ( $name:expr, $capacity: literal ) => {{
        thread_local! {
            static XCP_EVENT__: std::cell::Cell<XcpEvent> = const { std::cell::Cell::new(XcpEvent::UNDEFINED) }
        }
        if XCP_EVENT__.get() == XcpEvent::UNDEFINED {
            XCP_EVENT__.set(Xcp::get().create_event_ext($name, true));
        }
        DaqEvent::<$capacity>::new_from(&XCP_EVENT__.get())
    }};
    ( $name:expr ) => {{
        thread_local! {
            static XCP_EVENT__: std::cell::Cell<XcpEvent> = const { std::cell::Cell::new(XcpEvent::UNDEFINED) }
        }
        if XCP_EVENT__.get() == XcpEvent::UNDEFINED {
            XCP_EVENT__.set(Xcp::get().create_event_ext($name, true));
        }
        DaqEvent::<256>::new_from(&XCP_EVENT__.get())
    }};
}

/// Capture the value of a variable with basic type for the given multi instance daq event
/// Register the given meta data once for each thread
/// Append an index to the variable name to distinguish between different threads
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_capture_instance {
    // name, event, comment, unit, factor, offset
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr, $factor:expr, $offset:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                $factor,
                $offset,
                $unit,
                $comment,
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};

    // name, event, comment, unit
    ( $id:ident, $daq_event:expr, $comment:expr, $unit:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                1.0,
                0.0,
                $unit,
                $comment,
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};

    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        let mut offset = DAQ_OFFSET__.get();
        if offset == -32768 {
            offset = $daq_event.add_capture(
                stringify!($id),
                std::mem::size_of_val(&$id),
                $id.get_type(),
                1, // x_dim
                1, // y_dim
                1.0,
                0.0,
                "",
                "",
                None,
            );
            DAQ_OFFSET__.set(offset)
        };
        $daq_event.capture(&($id.to_le_bytes()), offset);
    }};
}

/// Register a local variable with basic type for the given daq event once for each thread
/// Address will be relative to the stack frame position of event
/// No capture buffer required
#[allow(unused_macros)]
#[macro_export]
macro_rules! daq_register_instance {
    // name, event
    ( $id:ident, $daq_event:expr ) => {{
        thread_local! {
            static DAQ_OFFSET__: std::cell::Cell<i16> = const { std::cell::Cell::new(-32768) }
        }
        if DAQ_OFFSET__.get() == -32768 {
            DAQ_OFFSET__.set(0);
            $daq_event.add_stack(stringify!($id), &$id as *const _ as *const u8, $id.get_type(), 1, 1, 1.0, 0.0, "", "");
        };
    }};
}

//-----------------------------------------------------------------------------
// Test
// Tests for the daq types
//-----------------------------------------------------------------------------

#[cfg(test)]
mod daq_tests {

    #![allow(dead_code)]
    #![allow(unused_imports)]

    use super::*;

    use crate::reg::*;
    use crate::xcp::*;

    //-----------------------------------------------------------------------------
    // Test local variable register
    #[test]
    fn daq_register() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let event = daq_create_event!("TestEvent1");
        let mut counter1: u16 = 0;
        daq_register!(counter1, event);
        loop {
            counter1 += 1;
            {
                let mut counter2: u8 = 0;
                daq_register!(counter2, event);
                counter2 += 1;

                {
                    let mut counter3: u32 = 0;
                    daq_register!(counter3, event);
                    counter3 += 1;
                    {
                        let mut counter4: u64 = 0;
                        daq_register!(counter4, event);
                        counter4 += 1;

                        trace!("counter1={}", counter1);
                        trace!("counter2={}", counter2);
                        trace!("counter3={}", counter3);
                        trace!("counter4={}", counter4);
                    }
                }
            }
            event.trigger();
            if counter1 == 3 {
                break;
            }
        }
        xcp.write_a2l();
    }

    //-----------------------------------------------------------------------------
    // Test local variable capture
    #[test]
    fn daq_capture() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let mut event = daq_create_event!("TestEvent1", 15);
        let mut counter1: u16 = 0;
        loop {
            counter1 += 1;
            {
                let mut counter2: u8 = 0;
                counter2 += 1;
                {
                    let mut counter3: u32 = 0;
                    counter3 += 1;
                    {
                        let mut counter4: u64 = 0;
                        counter4 += 1;
                        daq_capture!(counter3, event);
                        daq_capture!(counter4, event);
                    }
                }
                daq_capture!(counter2, event);
            }
            daq_capture!(counter1, event);
            event.trigger();
            if counter1 == 3 {
                break;
            }
        }
        xcp.write_a2l();
        std::fs::remove_file("xcp_lite.a2h").ok();
        std::fs::remove_file("xcp_lite.a2l").ok();
    }

    //-----------------------------------------------------------------------------
    // Test A2L file generation for local variables
    #[test]
    fn test_a2l_local_variables_capture() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();

        let mut event1 = daq_create_event!("task1", 256);
        let mut event2 = daq_create_event_instance!("task2", 256);
        let mut event3 = daq_create_event_instance!("task3", 256);
        let mut event4 = daq_create_event_instance!("task4", 256);
        let channel1: f64 = 1.0;
        let channel2: f64 = 2.0;
        let channel3: f64 = 3.0;
        let channel4: f64 = 3.0;
        let channel5: f64 = 3.0;
        daq_capture!(channel1, event1, "comment", "unit", 2.0, 5.0);
        daq_capture!(channel2, event1, "comment", "unit", 2.0, 5.0);
        daq_capture_instance!(channel3, event4, "", "Volt");
        daq_capture_instance!(channel4, event3, "comment", "unit");
        daq_capture_instance!(channel5, event2, "comment", "unit", 2.0, 5.0);
        xcp.write_a2l();
        std::fs::remove_file("xcp_lite.a2h").ok();
        std::fs::remove_file("xcp_lite.a2l").ok();
    }
}
