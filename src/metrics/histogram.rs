use crate::XcpEvent;
use crate::registry::get_lock;
use crate::registry::*;
use crate::xcp::*;

/*
Histogram
- The histograms counter array is a measurement variable with x dimensions [N]
- When given it a fix axis with a physical unit and step size, the A2L writer will write it a a CHARACTERISTIC with an IF_DATA XCP fixed event
- Ii uses the conversion rule from step_us as conversion rule for the fix axis (conversion rules have the same name as their objects)
- All metric fields are measurement variables with addressing mode DYN
- This allow asyncronous read (polling) and write access to modify or reset the histogram state
*/

//-------------------------------------------------------------------------------------------------
// Macros

// Static state (mutex)
#[allow(unused_macros)]
#[macro_export]
macro_rules! metrics_histogram {
    ( $name: expr, $dim: expr, $step: expr ) => {{
        static __HISTOGRAM: parking_lot::Mutex<Histogram<$dim>> = parking_lot::Mutex::new(Histogram::<$dim> {
            name: "Histogram",
            event: XcpEvent::XCP_UNDEFINED_EVENT,
            reset: 0,
            step_us: 0,
            last_time_ns: 0,
            cycle_counter: 0,
            cycle_time_us: 0,
            cycle_time_max_us: 0,
            cycle_time_min_us: 0xFFFF_FFFF,
            cycle_time_distribution: [0; $dim],
        });
        let mut v = __HISTOGRAM.lock();
        if v.event == XcpEvent::XCP_UNDEFINED_EVENT {
            v.init($name, $step, Xcp::get().create_event_ext($name, false));
            v.register_fields();
        };
        v.trigger();
    }};
}

// Thread local state (lock-free)
#[allow(unused_macros)]
#[macro_export]
macro_rules! metrics_histogram_tli {
    ( $name: expr, $dim: expr, $step: expr ) => {{
        thread_local! {
            static __HISTOGRAM: std::cell::RefCell<Histogram<$dim>> =  std::cell::RefCell::new(Histogram::<$dim> {
                name: "Histogram",
                event: XcpEvent::XCP_UNDEFINED_EVENT,
                reset: 0,
                step_us: 0,
                last_time_ns: 0,
                cycle_counter: 0,
                cycle_time_us: 0,
                cycle_time_max_us: 0,
                cycle_time_min_us: 0xFFFF_FFFF,
                cycle_time_distribution: [0; $dim],
            });
        }
        __HISTOGRAM.with_borrow_mut(|v| {
            if v.event == XcpEvent::XCP_UNDEFINED_EVENT {
                v.init($name, $step, Xcp::get().create_event_ext($name, true));
                v.register_fields();
            }
            v.trigger();
        });
        __HISTOGRAM
    }};
}

// const AXIS: [u32; 25] = [
//     0, 25, 50, 75, 100, 250, 500, 750, 1000, 2500, 5000, 7500, 10000, 25000, 50000, 75000, 100000, 250000, 500000, 750000, 1000000, 2500000, 5000000, 7500000, 10000000,
// ];

//-------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct Histogram<const N: usize> {
    pub name: &'static str,
    pub event: XcpEvent,
    pub reset: u8,    // Reset flag
    pub step_us: u32, // Step size in microseconds
    pub last_time_ns: u64,
    pub cycle_counter: u32, // Cycle count
    pub cycle_time_us: u32, // Cycle time in microseconds
    pub cycle_time_max_us: u32,
    pub cycle_time_min_us: u32,
    pub cycle_time_distribution: [u32; N],
}

impl<const N: usize> Histogram<N> {
    // Helper function
    // Register a field in the histogram with the given name, offset, value type, and mc meta data
    // # Panics
    // if the field name is not unique
    // if the field offset is out of range, which should not happen
    fn register_field(&mut self, field_name: &'static str, field_offset: usize, mc_value_type: McValueType, mc_support_data: McSupportData) {
        let event_offset = std::mem::offset_of!(Histogram<N>, event);
        let offset: i64 = field_offset as i64 - event_offset as i64;

        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
            format!("{}.{}", self.name, field_name),
            McDimType::new(mc_value_type, 1, 1),
            mc_support_data,
            McAddress::new_event_dyn(self.event.get_id(), offset.try_into().unwrap()),
        );
    }

    /// Register all fields of a histogram as measurement variables with write access
    /// Status may be reset by setting the reset flag to 1 or by writing directly to the fields
    pub fn register_fields(&mut self) {
        // Histogram
        let event_offset = std::mem::offset_of!(Histogram<N>, event);
        let offset: i64 = (std::mem::offset_of!(Histogram<N>, cycle_time_distribution) as i64 - event_offset as i64);
        let mc_support_data = McSupportData::new(McObjectType::Measurement)
            .set_comment("Cycle time histogram")
            .set_x_axis_conv(Some(format!("{}.step_us", self.name))); // Conversion rule for increments
        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
            format!("{}.histogram", self.name),
            McDimType::new(McValueType::Ulong, N as u16, 1),
            mc_support_data,
            McAddress::new_event_dyn(self.event.get_id(), offset.try_into().unwrap()),
        );

        self.register_field(
            "step_us",
            std::mem::offset_of!(Histogram<N>, step_us),
            McValueType::Ulong,
            McSupportData::new(McObjectType::Measurement).set_linear(self.step_us as f64, 0.0, "us"),
        );
        self.register_field(
            "counter",
            std::mem::offset_of!(Histogram<N>, cycle_counter),
            McValueType::Ulong,
            McSupportData::new(McObjectType::Measurement).set_comment("Cycle counter"),
        );
        self.register_field(
            "cycle_time",
            std::mem::offset_of!(Histogram<N>, cycle_time_us),
            McValueType::Ulong,
            McSupportData::new(McObjectType::Measurement).set_comment("Cycle time").set_linear(0.000001, 0.0, "s"),
        );
        self.register_field(
            "cycle_time_max",
            std::mem::offset_of!(Histogram<N>, cycle_time_max_us),
            McValueType::Ulong,
            McSupportData::new(McObjectType::Measurement).set_comment("Cycle time maximum").set_unit("us"),
        );
        self.register_field(
            "cycle_time_min",
            std::mem::offset_of!(Histogram<N>, cycle_time_min_us),
            McValueType::Ulong,
            McSupportData::new(McObjectType::Measurement).set_comment("Cycle time minimum").set_unit("us"),
        );
        self.register_field(
            "reset",
            std::mem::offset_of!(Histogram<N>, reset),
            McValueType::Ubyte,
            McSupportData::new(McObjectType::Measurement).set_comment("Reset flag"),
        );
    }

    pub fn init(&mut self, name: &'static str, step_us: u32, event: XcpEvent) {
        assert!(N <= 1000, "Histogram size must be <= 1000");
        assert!(step_us > 0, "Step size must be > 0");
        self.name = name;
        self.event = event;
        self.reset = 0;
        self.step_us = step_us;
        self.last_time_ns = Xcp::get().get_clock();
        self.cycle_counter = 0;
        self.cycle_time_us = 0;
        self.cycle_time_max_us = 0;
        self.cycle_time_min_us = 0xFFFF_FFFF;
        self.cycle_time_distribution = [0; N];
    }

    /// Create a new histogram with the given name and step size in microseconds
    /// Create an event with the given name
    pub fn new(name: &'static str, step_us: u32) -> Self {
        assert!(N <= 1000, "Histogram size must be <= 1000");
        assert!(step_us > 0, "Step size must be > 0");
        Histogram {
            name,
            event: Xcp::get().create_event(name),
            reset: 0,
            step_us,
            last_time_ns: Xcp::get().get_clock(),
            cycle_counter: 0,
            cycle_time_us: 0,
            cycle_time_max_us: 0,
            cycle_time_min_us: 0xFFFF_FFFF,
            cycle_time_distribution: [0; N],
        }
    }

    /// Trigger the histogram event and update the aggregations
    pub fn trigger(&mut self) {
        let time_ns = Xcp::get().get_clock(); // Get the current time in nanoseconds
        self.cycle_time_us = ((time_ns - self.last_time_ns) / 1000) as u32; // Convert to microseconds
        self.last_time_ns = time_ns;

        self.cycle_counter += 1;

        if self.cycle_time_min_us > self.cycle_time_us {
            self.cycle_time_min_us = self.cycle_time_us;
        }
        if self.cycle_time_max_us < self.cycle_time_us {
            self.cycle_time_max_us = self.cycle_time_us;
        }

        let mut index = (self.cycle_time_us / self.step_us) as usize;
        if index >= self.cycle_time_distribution.len() {
            index = self.cycle_time_distribution.len() - 1;
        }
        self.cycle_time_distribution[index] += 1;

        // Reset
        if self.reset > 0 {
            self.reset();
            self.reset = 0;
        }

        // Test
        // if v.cycle_counter % 1024 == 0 {
        //     v.print();
        // }

        // Trigger the event, base address is the event itself
        unsafe {
            self.event.trigger_ext(&self.event as *const _ as *const u8);
        }
    }

    /// Reset the histogram data
    pub fn reset(&mut self) {
        self.cycle_counter = 0;
        self.cycle_time_max_us = 0;
        self.cycle_time_min_us = 0xFFFF_FFFF;
        self.cycle_time_distribution = [0; N];
    }

    // fn print(&self) {
    //     println!(
    //         "Histogram {}: count={}, max={}, min={}",
    //         self.name, self.cycle_counter, self.cycle_time_max_us, self.cycle_time_min_us
    //     );
    //     for i in 0..self.cycle_time_distribution.len() {
    //         println!(
    //             "  {:.3}ms: \t{}%",
    //             (i * self.step_us as usize) as f64 / 1000.0,
    //             (self.cycle_time_distribution[i] * 1000 / self.cycle_counter) as f64 / 10.0
    //         );
    //     }
    // }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod histogram_tests {

    #![allow(dead_code)]
    #![allow(unused_imports)]

    use super::*;

    #[test]
    fn test_histogram_macro() {
        let xcp = xcp_test::test_setup();

        for n in 0..10 {
            // Create a histogram with 10 bins and a step size of 1000 us
            metrics_histogram_tli!("test_histogram_tli", 10, 1000).with_borrow(|h| assert_eq!(h.cycle_counter, n + 1));
        }
        for _n in 0..10 {
            // Create a histogram with 10 bins and a step size of 1000 us
            metrics_histogram!("test_histogram", 10, 1000);
        }
        xcp.finalize_registry().unwrap(); // Write the A2L file
    }

    #[test]
    fn test_histogram_manual() {
        let xcp = xcp_test::test_setup();
        let mut histogram = Histogram::<100>::new("test_histogram", 50); // Histogram metric
        histogram.register_fields(); // Register the histogram fields
        histogram.trigger();
        assert!(histogram.cycle_counter == 1); // Check that the cycle counter is 1
        xcp.finalize_registry().unwrap(); // Write the A2L file
    }
}
