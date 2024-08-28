#![allow(dead_code)]

//----------------------------------------------------------------------------------------------
// Module cal_seg
// Calibration Segment

use std::{
    marker::PhantomData,
    ops::Deref,
    sync::{Arc, Mutex},
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::reg;
use crate::xcp;
use crate::{cal, RegDataTypeProperties};
use cal::CalPageTrait;
use xcp::Xcp;
use xcp::XcpCalPage;

//----------------------------------------------------------------------------------------------
// Manually add calibration page fields to a calibration segment description

#[derive(Debug, Clone, Copy)]
pub struct CalPageField {
    pub name: &'static str,
    pub datatype: reg::RegistryDataType,
    pub offset: u16,
    pub dim: (usize, usize),
    pub comment: Option<&'static str>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub unit: Option<&'static str>,
}

#[macro_export]
macro_rules! calseg_field {
    (   $name:ident.$field:ident ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        assert!(offset < 0x10000, "offset too large");
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset as u16,
            dim: (1, 1),
            comment: None,
            min: None,
            max: None,
            unit: None,
        }
    }};
    (   $name:ident.$field:ident, $comment:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        assert!(offset < 0x10000, "offset too large");
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset as u16,
            dim: (1, 1),
            comment: Some($comment),
            min: None,
            max: None,
            unit: None,
        }
    }};
    (   $name:ident.$field:ident, $unit:expr, $comment:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        assert!(offset < 0x10000, "offset too large");
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset as u16,
            dim: (1, 1),
            comment: Some($comment),
            min: None,
            max: None,
            unit: Some($unit),
        }
    }};
    (   $name:ident.$field:ident, $min:expr, $max:expr, $unit:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        assert!(offset < 0x10000, "offset too large");
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset as u16,
            dim: (1, 1),
            comment: None,
            min: Some($min as f64),
            max: Some($max as f64),
            unit: Some($unit),
        }
    }};
}

//----------------------------------------------------------------------------------------------
// Calibration parameter page wrapper for T with modification counter, init and freeze requests

#[derive(Debug, Copy, Clone)]
struct CalPage<T: CalPageTrait> {
    ctr: u16,
    init_request: bool,
    freeze_request: bool,
    page: T,
}

//----------------------------------------------------------------------------------------------

/// Thread safe calibration parameter page wrapper with interiour mutabiity by XCP
/// Each instance stores 2 copies of its inner data, the calibration page
/// One for each clone of the readers, a shared copy for the writer (XCP) and
/// a reference to the default values
/// Implements Deref to simplify usage
#[derive(Debug)]
pub struct CalSeg<T>
where
    T: CalPageTrait,
{
    index: usize,
    default_page: &'static T,
    ecu_page: Box<CalPage<T>>,
    xcp_page: Arc<Mutex<CalPage<T>>>,
    _not_send_sync_marker: PhantomData<*mut ()>,
}

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Create a calibration segment for a calibration parameter struct T (called page)  
    /// With a name and static const default values, which will be the "FLASH" page
    /// The mutable "RAM" page is initialized from name.json, if load_json==true and if it exists, otherwise with default
    /// CalSeg is Send and implements Clone, so clones can be savely send to other threads
    /// This comes with the cost of maintaining a shadow copy of the calibration page for each clone
    /// On calibration tool changes, sync copies the shadow (xcp_page) to the active page (ecu_page)
    ///
    /// # Panics
    /// If the name is not unique
    /// If the maximum number of calibration segments is reached, CANape supports a maximum of 255 calibration segments
    ///
    pub fn new(index: usize, init_page: T, default_page: &'static T) -> CalSeg<T> {
        CalSeg {
            index,
            default_page,
            ecu_page: Box::new(CalPage {
                // Heap allocation
                ctr: 0,
                init_request: false,
                freeze_request: false,
                page: init_page,
            }),
            xcp_page: Arc::new(Mutex::new(CalPage {
                // Heap allocation
                ctr: 0,
                init_request: false,
                freeze_request: false,
                page: init_page,
            })),
            _not_send_sync_marker: PhantomData,
        }
    }

    /// Manually add a field description
    #[allow(clippy::too_many_arguments)]
    pub fn add_field(&self, field: CalPageField) -> &CalSeg<T> {
        trace!("add_field: {:?}", field);
        let datatype = field.datatype;
        let unit = if field.unit.is_some() { field.unit.unwrap() } else { "" };
        let comment = if field.comment.is_some() { field.comment.unwrap() } else { "" };
        let min = if field.min.is_some() { field.min.unwrap() } else { datatype.get_min() };
        let max = if field.max.is_some() { field.max.unwrap() } else { datatype.get_max() };
        let c = crate::reg::RegistryCharacteristic::new(
            Some(self.get_name()),
            format!("{}.{}", self.get_name(), field.name),
            datatype,
            comment,
            min,
            max,
            unit,
            field.dim.0,
            field.dim.1,
            field.offset as u64,
        );

        Xcp::get().get_registry().lock().unwrap().add_characteristic(c);

        self
    }

    /// Get the calibration segment clone count
    pub fn get_clone_count(&self) -> u16 {
        Arc::strong_count(&self.xcp_page) as u16
    }

    /// Sync the calibration segment
    /// If calibration changes from XCP tool happened since last sync, copy the xcp page to the ecu page
    /// Handle freeze and init operations on request here
    /// # Returns
    /// true, if the calibration segment was modified
    pub fn sync(&self) -> bool {
        let _xcp = Xcp::get();
        let mut modified = false;

        // Check for modifications and copy xcp_page to ecu_page, when active page is "RAM"
        /*if xcp.get_xcp_cal_page() == XcpCalPage::Ram */
        {
            // @@@@ ToDo: Avoid the lock, when there is no pending modification for the XCP page
            {
                let mut xcp_page = self.xcp_page.lock().unwrap();

                // Freeze - save xcp page to json file
                #[cfg(feature = "json")]
                if xcp_page.freeze_request {
                    xcp_page.freeze_request = false;
                    trace!("freeze: {})", self.get_name(),);
                    // Reinitialize the calibration segment from default page
                    let path = format!("{}.json", self.get_name());
                    self.ecu_page.page.save_to_file(&path);
                }

                // Init - copy the default calibration page back to xcp page to reset it to default values
                if xcp_page.init_request {
                    xcp_page.init_request = false;
                    // @@@@ unsafe - Implementation of init cal page in sync() with non mut self
                    unsafe {
                        trace!("init: {}: default_page => xcp_page ({})", self.get_name(), xcp_page.ctr,);

                        let src_ptr = self.default_page as *const T;
                        let dst_ptr = &xcp_page.page as *const _ as *mut T;
                        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);

                        // Increment the modification counter to distribute the new xcp page to all clones
                        xcp_page.ctr += 1;
                        modified = true;
                    }
                }

                // Copy shared (ctr,xcp_page) to (ctr,ecu_page) in this clone of the calibration segment
                if xcp_page.ctr != self.ecu_page.ctr {
                    trace!(
                        "sync: {}-{:04X}: xcp_page ({}) => ecu_page ({})",
                        self.get_name(),
                        self.ecu_page.as_ref() as *const _ as u16,
                        xcp_page.ctr,
                        self.ecu_page.ctr
                    );
                    // @@@@ unsafe - Copy xcp_page to ecu_page
                    unsafe {
                        let dst_ptr: *mut u8 = self.ecu_page.as_ref() as *const _ as *mut u8;
                        let src_ptr: *const u8 = &*xcp_page as *const _ as *const u8;
                        let size: usize = std::mem::size_of::<(usize, T)>();
                        core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, size);
                    }
                    modified = true;
                }
            }
        }
        modified
    }
}

//----------------------------------------------------------------------------------------------
// Trait CalSegTrait

pub trait CalSegTrait
where
    Self: Send,
{
    fn get_name(&self) -> &'static str;
    fn set_index(&mut self, index: usize);
    fn get_index(&self) -> usize;

    /// Set freeze and init requests
    fn set_freeze_request(&self);
    fn set_init_request(&self);

    /// Read from xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// Memory access is unsafe, src checked to be inside a calibration segment
    /// src is a pointer to the destination data in XCPlite
    unsafe fn read(&self, offset: u16, len: u8, src: *mut u8) -> bool;

    /// Write to xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// Memory access is unsafe, dst checked to be inside a calibration segment
    /// src is a pointer to the source data in XCPlite
    unsafe fn write(&self, offset: u16, len: u8, src: *const u8, delay: u8) -> bool;

    // Flush delayed modifications
    fn flush(&self);
}

impl<T> CalSegTrait for CalSeg<T>
where
    T: CalPageTrait,
{
    fn get_name(&self) -> &'static str {
        Xcp::get().get_calseg_name(self.index)
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }

    fn get_index(&self) -> usize {
        self.index
    }
    fn set_freeze_request(&self) {
        let mut m = self.xcp_page.lock().unwrap();
        m.freeze_request = true;
    }

    fn set_init_request(&self) {
        let mut m = self.xcp_page.lock().unwrap();
        m.init_request = true;
    }

    unsafe fn read(&self, offset: u16, len: u8, dst: *mut u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let xcp_page = self.xcp_page.lock().unwrap();
            let src: *const u8 = (&xcp_page.page as *const _ as *const u8).add(offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, len as usize);
            true
        } else {
            let src: *const u8 = (self.default_page as *const _ as *const u8).add(offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, len as usize);
            true
        }
    }

    unsafe fn write(&self, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let mut xcp_page = self.xcp_page.lock().unwrap();
            let dst: *mut u8 = (&xcp_page.page as *const _ as *mut u8).add(offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, len as usize);
            if delay == 0 {
                // Increment modification counter
                xcp_page.ctr = xcp_page.ctr.wrapping_add(1);
            }
            true
        } else {
            false // Write to default page is not allowed
        }
    }

    fn flush(&self) {
        let mut xcp_page = self.xcp_page.lock().unwrap();
        xcp_page.ctr = xcp_page.ctr.wrapping_add(1); // Increment modification counter
    }
}

//----------------------------------------------------------------------------------------------
// Implement Deref for CalSegSync

impl<T> Deref for CalSeg<T>
where
    T: CalPageTrait,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let xcp = Xcp::get();
        // Deref to currently active page
        match xcp.get_ecu_cal_page() {
            XcpCalPage::Ram => &self.ecu_page.page,
            _ => self.default_page,
        }
    }
}

//----------------------------------------------------------------------------------------------
// Implement DerefMut for CalSegSync
// @@@@ For testing only
// Deref to XCP page and increment the modification counter
// This is undefined behaviour, because the reference to XCP data page will escape from its mutex
// impl<T> DerefMut for CalSeg<T>
// where
//     T: CalPageTrait,
// {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         warn!("Unsafe deref mut to XCP page of {}, this is undefined behaviour !!", self.get_name());
//         let mut p = self.xcp_page.lock().unwrap();
//         p.ctr = p.ctr.wrapping_add(1);
//         let r: *mut T = &mut p.page;
//         unsafe { &mut *r }
//     }
// }

//----------------------------------------------------------------------------------------------
// Implement Clone for CalSegSync

impl<T> Clone for CalSeg<T>
where
    T: CalPageTrait,
{
    fn clone(&self) -> Self {
        CalSeg {
            index: self.index,
            default_page: self.default_page,      // &T
            ecu_page: self.ecu_page.clone(),      // Clone for each thread
            xcp_page: Arc::clone(&self.xcp_page), // Share Arc<Mutex<T>>
            _not_send_sync_marker: PhantomData,
        }
    }
}

//----------------------------------------------------------------------------------------------
// Implement Drop for CalSeg
// Create a warning, if a CalSeg is completely dropped, which usually makes no sense while the XCP server is running

// impl<T> Drop for CalSeg<T>
// where
//     T: CalPageTrait,
// {
//     fn drop(&mut self) {
//         let clone_count = self.get_clone_count();
//         if clone_count > 1 {
//             // Warn if the application drops its last clone of a CalSeg
//             // The only remaining clones is in the XCP calseg_list
//             if clone_count == 2 {
//                 warn!("CalSeg {} dropped by application", self.get_name());
//             }
//         }
//     }
// }

//----------------------------------------------------------------------------------------------
// Send marker

// The Send marker trait indicates that ownership of the type can be transferred to a different thread.
// The Sync marker trait would indicates that it is safe to share references to CalSeg between threads, which is not the case.

/// Send marker for CalSeg
/// CalSeg is not Sync, but Send
/// # Safety
/// This is safe, because CalSeg would be Send and Sync, but its disabled by PhantomData
/// Send is reimplemented here
/// Sync stays disabled, because this would allow to call calseg.sync() from multiple threads with references to the same CalSeg
// @@@@ unsafe - Implementation of Send marker for CalSeg
unsafe impl<T> Send for CalSeg<T> where T: CalPageTrait {}

//----------------------------------------------------------------------------------------------
// Test
// Tests for the calibration segment
//----------------------------------------------------------------------------------------------

#[cfg(test)]
mod cal_tests {

    #![allow(dead_code)]
    #![allow(unused_imports)]

    use super::*;
    use crate::reg::RegistryCharacteristic;
    use crate::xcp;
    use crate::xcplib;
    use reg::Registry;
    use xcp_type_description::prelude::*;

    use xcp::*;
    use xcp_type_description_derive::XcpTypeDescription;

    use serde::{Deserialize, Serialize};
    use std::sync::{mpsc, mpsc::Sender, Arc, Mutex, Once, RwLock};
    use std::thread;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    //-----------------------------------------------------------------------------
    // Test Types

    fn is_copy<T: Sized + Copy>() {}
    fn is_send<T: Sized + Send>() {}
    fn is_sync<T: Sized + Sync>() {}
    fn is_clone<T: Sized + Clone>() {}
    fn is_send_clone<T: Sized + Send + Clone>() {}
    fn is_send_sync<T: Sized + Send + Sync>() {}

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
    struct CalPage0 {
        stop: bool,
    }

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
    struct CalPage4 {
        test: u8,
    }

    fn task_calseg(cal_seg: CalSeg<CalPage0>) {
        info!("task_calseg");
        loop {
            if cal_seg.stop {
                break;
            }
            cal_seg.sync();
        }
        info!("task_calseg end");
    }

    fn task_mutex(stop: Arc<Mutex<&bool>>) {
        info!("task_stop");
        loop {
            if **(stop.lock().unwrap()) {
                break;
            }
        }
        info!("task_calseg end");
    }

    #[test]
    fn test_calibration_segment_basics() {
        xcp_test::test_setup(log::LevelFilter::Info);

        is_sync::<Xcp>();
        is_sync::<XcpEvent>();
        //is_sync::<DaqEvent>();
        is_copy::<CalPage1>();
        is_send::<CalPage1>();
        //is_sync::<CalPage>();
        is_send::<CalSeg<CalPage1>>();
        //is_sync::<CalSeg<CalPage>>(); // CalSeg is not sync !
        is_clone::<CalSeg<CalPage1>>();
        //is_copy::<CalSeg<CalPage0>>(); // CalSeg is not copy

        const CAL_PAGE: CalPage0 = CalPage0 { stop: true };

        // Intended use
        let cal_seg1 = Xcp::create_calseg("calseg1", &CAL_PAGE, false);
        cal_seg1.sync();
        assert!(cal_seg1.stop);
        let c1 = CalSeg::clone(&cal_seg1);
        let c2 = CalSeg::clone(&cal_seg1);
        assert!(cal_seg1.get_clone_count() == 4); // 2 explicit clones, 1 for Xcp calseg_list and the original
        let t1 = thread::spawn(move || {
            task_calseg(c1);
        });

        let t2 = thread::spawn(move || {
            task_calseg(c2);
        });
        t1.join().unwrap();
        t2.join().unwrap();
        let size = std::mem::size_of::<CalSeg<CalPage1>>();
        let clones = cal_seg1.get_clone_count();
        info!("CalSeg: {} size = {} bytes, clone_count = {}", cal_seg1.get_name(), size, clones);
        assert_eq!(size, 32);
        assert!(clones == 2); // 2 clones move to threads and dropped
        drop(cal_seg1);

        // Illegal use
        // Creating references to interiour mutable calibration parameters
        // This can not result in undefined behaviour, because the reference can never escape this thread
        // The mutation (value change and page switch) always happens in cal_seg.sync in this thread
        // The only effect would be, that we hold a reference to the wrong page, as demonstrated here
        const CAL_PAGE2: CalPage4 = CalPage4 { test: 0x55 }; // FLASH
        let cal_page2 = CalPage4 { test: 0xAA }; // RAM
        cal_page2.save_to_file("calseg2.json");
        let cal_seg2 = Xcp::create_calseg("calseg2", &CAL_PAGE2, true);
        Xcp::get().set_ecu_cal_page(XcpCalPage::Ram);
        let r = &cal_seg2.test;
        assert_eq!(*r, 0xAA); // RAM page
        assert_eq!(cal_seg2.test, 0xAA); // RAM page
        Xcp::get().set_ecu_cal_page(XcpCalPage::Flash);
        assert_eq!(*r, 0xAA); // RAM page
        assert_eq!(cal_seg2.test, 0x55); // FLASH page
        std::fs::remove_file("calseg2.json").ok();
    }

    //-----------------------------------------------------------------------------
    // Test file read and write of a cal_seg

    #[test]
    fn test_calibration_segment_persistence() {
        xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
        struct CalPage {
            test_byte: u8,
            test_short: u16,
            ampl: f64,
            period: f64,
        }

        const CAL_PAR_FLASH: CalPage = CalPage {
            test_byte: 0xAA,
            test_short: 0x1234,
            ampl: 100.0,
            period: 5.0,
        };
        static CAL_PAR_RAM: CalPage = CalPage {
            test_byte: 0x55,
            test_short: 0,
            ampl: 50.0,
            period: 0.,
        };

        let xcp = Xcp::get();

        // Create a test_cal_page.json file with values from CAL_PAR_RAM
        let mut_page: Box<CalPage> = Box::new(CAL_PAR_RAM);
        mut_page.save_to_file("test_cal_seg.json");

        // Create a cal_seg with a mut_page from file test_cal_seg.json aka CAL_PAR_RAM, and a default page from CAL_PAR_FLASH
        let cal_seg = &Xcp::create_calseg("test_cal_seg", &CAL_PAR_FLASH, true);
        let cal_seg1 = cal_seg.clone();
        let cal_seg2 = cal_seg.clone();

        assert_eq!(cal_seg.test_byte, 0x55, "test_byte != 0x55, default is RAM");
        let r = &cal_seg.test_byte;
        assert_eq!(*r, 0x55, "&test_byte != 0x55, default is RAM");
        xcp.set_ecu_cal_page(XcpCalPage::Flash);
        assert_eq!(cal_seg.test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        assert_eq!(cal_seg1.test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        assert_eq!(cal_seg2.test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        assert_eq!(*r, 0x55, "&test_byte != 0x55, reference to RAM"); // @@@@ Note: References are legal, not affected by switch
        xcp.set_ecu_cal_page(XcpCalPage::Ram);
        assert_eq!(cal_seg.test_byte, 0x55, "test_byte != 0x55 from RAM");
        assert_eq!(cal_seg1.test_byte, 0x55, "test_byte != 0x55 from RAM");
        assert_eq!(cal_seg2.test_byte, 0x55, "test_byte != 0x55 from RAM");
        drop(cal_seg2);
        drop(cal_seg1);
        let _ = cal_seg;

        std::fs::remove_file("test_cal_seg.json").ok();
    }

    //-----------------------------------------------------------------------------
    // Test cal page switching

    #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
    struct CalPage1 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
    struct CalPage2 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
    struct CalPage3 {
        a: u32,
        b: u32,
        c: u32,
    }

    static FLASH_PAGE1: CalPage1 = CalPage1 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE2: CalPage2 = CalPage2 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE3: CalPage3 = CalPage3 { a: 2, b: 4, c: 6 };

    macro_rules! test_is_mut {
        ( $s:ident ) => {
            if $s.a != 1 || $s.b != 3 || $s.c != 5 {
                panic!("test_is_mut: failed, s.a!=1 || s.b!=3 || s.c!=5");
            }
            trace!("test_is_mut: a={}, b={}, c={}", $s.a, $s.b, $s.c);
        };
    }

    macro_rules! test_is_default {
        ( $s:ident ) => {
            if $s.a != 2 || $s.b != 4 || $s.c != 6 {
                panic!("test_is_default: failed, s.a!=2 || s.b!=4 || s.c!=6");
            }
            trace!("test_is_default: a={}, b={}, c={}", $s.a, $s.b, $s.c);
        };
    }

    #[test]
    fn test_cal_page_switch() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();
        let mut_page: CalPage2 = CalPage2 { a: 1, b: 3, c: 5 };
        mut_page.save_to_file("test1.json");
        mut_page.save_to_file("test2.json");
        let cal_seg = Xcp::create_calseg("test1", &FLASH_PAGE2, true); // active page is RAM from test1.json
        assert_eq!(xcp.get_ecu_cal_page(), XcpCalPage::Ram, "XCP should be on RAM page here, there is no independant page switching yet");
        test_is_mut!(cal_seg); // Default page must be mut_page
        xcp.set_ecu_cal_page(XcpCalPage::Flash); // Simulate a set cal page to default from XCP master
        cal_seg.sync();
        test_is_default!(cal_seg);
        xcp.set_ecu_cal_page(XcpCalPage::Ram); // Switch back to ram
        cal_seg.sync();
        test_is_mut!(cal_seg);
        // Check if cal page switching works in a loop where the compiler might optimize the cal_page values
        for i in 0..10 {
            if i <= 50 {
                if cal_seg.a != 1 {
                    unreachable!();
                };
            } else if cal_seg.a != 2 {
                unreachable!();
            }
            if i == 50 {
                xcp.set_ecu_cal_page(XcpCalPage::Flash); // Switch to default
                cal_seg.sync();
            }
        }
        std::fs::remove_file("test1.json").ok();
        std::fs::remove_file("test2.json").ok();
    }

    //-----------------------------------------------------------------------------
    // Test cal page freeze
    // @@@@ Bug: Test fails occasionally
    #[test]
    fn test_cal_page_freeze() {
        xcp_test::test_setup(log::LevelFilter::Warn);
        let _xcp = Xcp::get();

        assert!(std::mem::size_of::<CalPage1>() == 12);
        assert!(std::mem::size_of::<CalPage2>() == 12);
        assert!(std::mem::size_of::<CalPage3>() == 12);

        let mut_page1: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
        mut_page1.save_to_file("test1.json");

        // Create calseg1 from def
        let calseg1 = Xcp::create_calseg("test1", &FLASH_PAGE1, true);
        test_is_mut!(calseg1);

        // Freeze calseg1 to new test1.json
        std::fs::remove_file("test1.json").ok();
        xcp_test::test_freeze_cal(); // Save mut_page to file "test1.json"
        calseg1.sync();

        // Create calseg2 from freeze file test1.json of calseg1
        std::fs::copy("test1.json", "test2.json").unwrap();
        let calseg2 = Xcp::create_calseg("test2", &FLASH_PAGE2, true);
        test_is_mut!(calseg2);

        std::fs::remove_file("test1.json").ok();
        std::fs::remove_file("test2.json").ok();
    }

    //-----------------------------------------------------------------------------
    // Test cal page trait compiler errors

    #[test]
    fn test_cal_page_trait() {
        xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
        struct Page1 {
            a: u32,
        }

        const PAGE1: Page1 = Page1 { a: 1 };
        #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
        struct Page2 {
            b: u32,
        }

        const PAGE2: Page2 = Page2 { b: 1 };
        #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
        struct Page3 {
            c: u32,
        }

        const PAGE3: Page3 = Page3 { c: 1 };

        let s1 = &Xcp::create_calseg("test1", &PAGE1, true);
        let s2 = &Xcp::create_calseg("test2", &PAGE2, true);
        let s3 = &Xcp::create_calseg("test3", &PAGE3, true);

        info!("s1: {}", s1.get_name());
        info!("s2: {}", s2.get_name());
        info!("d3: {}", s3.get_name());

        let d1: Box<dyn CalSegTrait> = Box::new(s1.clone());
        info!("d1: {}", d1.get_name());
        is_send::<Box<dyn CalSegTrait + Send>>();

        #[allow(clippy::vec_init_then_push)]
        let mut v: Vec<Box<dyn CalSegTrait>> = Vec::new();
        v.push(Box::new(s1.clone()));
        v.push(Box::new(s2.clone()));
        v.push(Box::new(s3.clone()));
        v.push(Box::new(s3.clone()));

        for (i, s) in v.iter().enumerate() {
            info!(" {}: {}", i, s.get_name());
        }

        let a: Arc<Mutex<Vec<Box<dyn CalSegTrait>>>> = Arc::new(Mutex::new(Vec::new()));
        {
            let mut v = a.lock().unwrap();
            v.push(Box::new(s1.clone()));
            v.push(Box::new(s2.clone()));
            v.push(Box::new(s3.clone()));
            v.push(Box::new(s3.clone()));
        }

        let c = a.clone();
        let t = thread::spawn(move || {
            let v = c.lock().unwrap();
            for (i, s) in v.iter().enumerate() {
                info!("thread {}: {}", i, s.get_name());
            }
        });
        t.join().unwrap();
    }

    //-----------------------------------------------------------------------------
    // Test attribute macros

    #[test]
    fn test_attribute_macros() {
        xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(Debug, Copy, Clone, Serialize, Deserialize, XcpTypeDescription)]
        struct CalPage {
            #[type_description(comment = "Comment")]
            #[type_description(unit = "Unit")]
            #[type_description(min = "0")]
            #[type_description(max = "100")]
            a: u32,
            b: u32,
            curve: [f64; 16],  // This will be a CURVE type (1 dimension)
            map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)
        }
        const CAL_PAGE: CalPage = CalPage {
            a: 1,
            b: 2,
            curve: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
            map: [
                [0, 0, 0, 0, 0, 0, 0, 1, 2],
                [0, 0, 0, 0, 0, 0, 0, 2, 3],
                [0, 0, 0, 0, 0, 1, 1, 2, 3],
                [0, 0, 0, 0, 1, 1, 2, 3, 4],
                [0, 0, 1, 1, 2, 3, 4, 5, 7],
                [0, 1, 1, 1, 2, 4, 6, 8, 9],
                [0, 1, 1, 2, 4, 5, 8, 9, 10],
                [0, 1, 1, 3, 5, 8, 9, 10, 10],
            ],
        };

        let calseg = &Xcp::create_calseg("calseg", &CAL_PAGE, false);
        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().unwrap().find_characteristic("CalPage.a").unwrap().clone();

        assert_eq!(calseg.get_name(), "calseg");
        assert_eq!(c.comment(), "Comment");
        assert_eq!(c.unit(), "Unit");
        assert_eq!(c.min(), 0.0);
        assert_eq!(c.max(), 100.0);
        assert_eq!(c.x_dim(), 1);
        assert_eq!(c.y_dim(), 1);
        assert_eq!(c.addr_offset(), 200);
        assert_eq!(c.datatype(), reg::RegistryDataType::Ulong);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().unwrap().find_characteristic("CalPage.b").unwrap().clone();
        assert_eq!(c.addr_offset(), 204);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().unwrap().find_characteristic("CalPage.curve").unwrap().clone();
        assert_eq!(c.addr_offset(), 0);
        assert_eq!(c.x_dim(), 16);
        assert_eq!(c.y_dim(), 1);

        let c: RegistryCharacteristic = Xcp::get().get_registry().lock().unwrap().find_characteristic("CalPage.map").unwrap().clone();
        assert_eq!(c.addr_offset(), 128);
        assert_eq!(c.x_dim(), 8);
        assert_eq!(c.y_dim(), 9);
    }
}
