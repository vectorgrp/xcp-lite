#![allow(dead_code)]

//----------------------------------------------------------------------------------------------
// Module cal_seg
// Calibration Segment

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use super::RegisterFieldsTrait;
use crate::reg;
use crate::xcp;
use parking_lot::Mutex;
use std::{marker::PhantomData, ops::Deref, sync::Arc};
use xcp::Xcp;
use xcp::XcpCalPage;

//----------------------------------------------------------------------------------------------
// Manually add calibration page fields to a calibration segment description

/// Calibration page field description
/// Glue used by the calseg_field macro to manually add a field to a calibration segment
/// # example
/// '''
/// const CAL_PAGE: CalPage = CalPage { cycle_time_ms: MAINLOOP_CYCLE_TIME };
/// let calseg = xcp.add_calseg("CalPage", &CAL_PAGE );
/// calseg.add_field(calseg_field!(CAL_PAGE.cycle_time_ms, "ms", "main task cycle time"));
/// '''

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

/// Format a calibration segment field description to be added with CalSeg::add_field
#[macro_export]
macro_rules! calseg_field {
    (   $name:ident.$field:ident ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);

        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset.try_into().expect("offset too large"),
            dim: (1, 1),
            comment: None,
            min: None,
            max: None,
            unit: None,
        }
    }};
    (   $name:ident.$field:ident, $comment:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset.try_into().expect("offset too large"),
            dim: (1, 1),
            comment: Some($comment),
            min: None,
            max: None,
            unit: None,
        }
    }};
    (   $name:ident.$field:ident, $unit:expr, $comment:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset.try_into().expect("offset too large"),
            dim: (1, 1),
            comment: Some($comment),
            min: None,
            max: None,
            unit: Some($unit),
        }
    }};
    (   $name:ident.$field:ident, $min:expr, $max:expr, $unit:expr ) => {{
        let offset = (&($name.$field) as *const _ as *const u8 as u64).wrapping_sub(&$name as *const _ as *const u8 as u64);
        CalPageField {
            name: stringify!($field),
            datatype: $name.$field.get_type(),
            offset: offset.try_into().expect("offset too large"),
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

//-----------------------------------------------------------------------------
// CalPageTrait

// Calibration pages must be Sized + Send + Sync + Copy + Clone + 'static

#[cfg(feature = "serde")]
pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + serde::Serialize + serde::de::DeserializeOwned,
{
}

#[cfg(not(feature = "serde"))]
pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static,
{
}

// Implement CalPageTrait for all types that may be a calibration page
#[cfg(feature = "serde")]
impl<T> CalPageTrait for T where T: Sized + Send + Sync + Copy + Clone + 'static + serde::Serialize + serde::de::DeserializeOwned {}

#[cfg(not(feature = "serde"))]
impl<T> CalPageTrait for T where T: Sized + Send + Sync + Copy + Clone + 'static {}

//----------------------------------------------------------------------------------------------
// CalSeg

/// Thread safe calibration parameter page wrapper with interiour mutabiity by XCP
/// Each instance stores 2 copies of its inner data, the calibration page
/// One for each clone of the readers, a shared copy for the writer (XCP) and
/// a reference to the default values
/// Implements Deref to simplify usage, is send, not sync and implements copy and clone
///

#[derive(Debug)]
pub struct CalSeg<T>
where
    T: CalPageTrait,
{
    index: usize,
    default_page: &'static T,
    ecu_page: Box<CalPage<T>>,
    xcp_page: Arc<Mutex<CalPage<T>>>,
    _not_sync_marker: PhantomData<std::cell::Cell<()>>, // CalSeg is send, not sync
}

// Impl register_fields for types which implement RegisterFieldsTrait
impl<T> CalSeg<T>
where
    T: CalPageTrait + RegisterFieldsTrait,
{
    /// Register all fields of a calibration segment in the registry
    /// Requires the calibration page to implement XcpTypeDescription
    pub fn register_fields(&self) -> &Self {
        self.default_page.register_fields(self.get_name());
        self
    }
}

// Impl load and save for type which implement serde::Serialize and serde::de::DeserializeOwned
#[cfg(feature = "serde")]
impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Load a calibration segment from json file
    /// Requires the calibration page type to implement serde::Serialize + serde::de::DeserializeOwned

    pub fn load<P: AsRef<std::path::Path>>(&self, filename: P) -> Result<(), std::io::Error> {
        let path = filename.as_ref();
        info!("Load {} from file {} ", self.get_name(), path.display());
        if let Ok(file) = std::fs::File::open(path) {
            let reader = std::io::BufReader::new(file);
            let page = serde_json::from_reader::<_, T>(reader)?;
            self.xcp_page.lock().page = page;
            self.xcp_page.lock().ctr += 1;
            self.sync();
            Ok(())
        } else {
            warn!("File not found: {}", path.display());
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("File not found: {}", path.display())))
        }
    }

    /// Write a calibrationsegment to json file
    /// Requires the calibration page type to implement serde::Serialize + serde::de::DeserializeOwned
    pub fn save<P: AsRef<std::path::Path>>(&self, filename: P) -> Result<(), std::io::Error> {
        let path = filename.as_ref();
        info!("Save {} to file {}", self.get_name(), path.display());
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        let s = serde_json::to_string(&self.xcp_page.lock().page).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("serde_json::to_string failed: {}", e)))?;
        std::io::Write::write_all(&mut writer, s.as_ref())?;
        Ok(())
    }
}

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Create a calibration segment for a calibration parameter struct T (called calibration page type)
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
                ctr: 0,
                init_request: false,
                freeze_request: false,
                page: init_page,
            }),
            xcp_page: Arc::new(Mutex::new(CalPage {
                ctr: 0,
                init_request: false,
                freeze_request: false,
                page: init_page,
            })),
            //_not_send_sync_marker: PhantomData,
            _not_sync_marker: PhantomData,
        }
    }

    /// Get the calibration segment name
    pub fn get_name(&self) -> &'static str {
        Xcp::get().get_calseg_name(self.index)
    }

    /// Manually add a field description
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

        Xcp::get().get_registry().lock().add_characteristic(c).expect("Duplicate");

        self
    }

    /// Get the calibration segment clone count
    pub fn get_clone_count(&self) -> usize {
        Arc::strong_count(&self.xcp_page)
    }

    /// Consistent read access to the calibration segment while the lock guard is held
    pub fn read_lock(&self) -> ReadLockGuard<'_, T> {
        self.sync();
        // page swap logic inside deref
        let xcp_or_default_page = &**self;
        ReadLockGuard { page: xcp_or_default_page }
    }

    /// Sync the calibration segment
    /// If calibration changes from XCP tool happened since last sync, copy the xcp page to the ecu page
    /// Handle freeze and init operations on request here
    /// # Returns
    /// true, if the calibration segment was modified
    pub fn sync(&self) -> bool {
        let mut modified = false;

        // Check for modifications and copy xcp_page to ecu_page, when active page is "RAM"
        // let xcp = Xcp::get();
        // if xcp.get_xcp_cal_page() == XcpCalPage::Ram
        {
            // @@@@ ToDo: Avoid the lock, when there is no pending modification for the XCP page
            let mut xcp_page = self.xcp_page.lock();

            // Freeze - save xcp page to json file
            // @@@@ don't panic, if the file can't be written
            #[cfg(feature = "serde")]
            if xcp_page.freeze_request {
                xcp_page.freeze_request = false;
                info!("freeze: save {}.json)", self.get_name());

                let mut path = std::path::PathBuf::from(self.get_name());
                path.set_extension("json");

                let file = std::fs::File::create(path).unwrap();
                let mut writer = std::io::BufWriter::new(file);
                let s = serde_json::to_string(&xcp_page.page)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("serde_json::to_string failed: {}", e)))
                    .unwrap();
                std::io::Write::write_all(&mut writer, s.as_ref()).unwrap();
            }

            // Init - copy the default calibration page back to xcp page to reset it to default values
            if xcp_page.init_request {
                xcp_page.init_request = false;
                // @@@@ Unsafe - Implementation of init cal page in sync() with non mut self
                unsafe {
                    info!("init: {}: default_page => xcp_page ({})", self.get_name(), xcp_page.ctr,);

                    let src_ptr = self.default_page as *const T;
                    #[allow(clippy::ptr_cast_constness)]
                    let dst_ptr = &xcp_page.page as *const _ as *mut T;
                    core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);
                }

                // Increment the modification counter to distribute the new xcp page to all clones
                xcp_page.ctr += 1;
            }

            // Sync - Copy shared (ctr,xcp_page) to (ctr,ecu_page) in this clone of the calibration segment
            if xcp_page.ctr != self.ecu_page.ctr {
                trace!("sync: {}: xcp_page ({}) => ecu_page ({})", self.get_name(), xcp_page.ctr, self.ecu_page.ctr);
                // @@@@ Unsafe - Copy xcp_page to ecu_page
                unsafe {
                    let dst_ptr: *mut u8 = self.ecu_page.as_ref() as *const _ as *mut u8;
                    let src_ptr: *const u8 = &*xcp_page as *const _ as *const u8;
                    let size: usize = std::mem::size_of::<(usize, T)>();
                    core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, size);
                }
                modified = true;
            }

            modified
        }
    }
}

//----------------------------------------------------------------------------------------------
// Trait CalSegTrait

pub trait CalSegTrait
where
    Self: Send,
{
    // Get the calibration segment name
    fn get_name(&self) -> &'static str;

    // Set the calibration segment index
    fn set_index(&mut self, index: usize);

    // Get the calibration segment index
    fn get_index(&self) -> usize;

    // Set freeze requests
    fn set_freeze_request(&self);

    // Set init request
    fn set_init_request(&self);

    // Read from xcp_page or default_page depending on the active XCP page
    // # Safety
    // dst must be valid
    // @@@@ - Unsafe function
    unsafe fn read(&self, offset: u16, len: u8, dst: *mut u8) -> bool;

    // Write to xcp_page
    // # Safety
    // src must be valid
    // @@@@ - Unsafe function
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
        self.xcp_page.lock().freeze_request = true;
    }

    fn set_init_request(&self) {
        self.xcp_page.lock().init_request = true;
    }

    // @@@@ Unsafe - function
    unsafe fn read(&self, offset: u16, len: u8, dst: *mut u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let xcp_page = self.xcp_page.lock();
            let src: *const u8 = (&xcp_page.page as *const _ as *const u8).add(offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, len as usize);
            true
        } else {
            let src: *const u8 = (self.default_page as *const _ as *const u8).add(offset as usize);
            core::ptr::copy_nonoverlapping(src, dst, len as usize);
            true
        }
    }

    // @@@@ Unsafe - function
    unsafe fn write(&self, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let mut xcp_page = self.xcp_page.lock(); // .unwrap(); // std::sync::MutexGuard
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
        let mut xcp_page = self.xcp_page.lock();
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

    // Deref to currently active page
    #[inline]
    fn deref(&self) -> &Self::Target {
        if xcp::XCP.ecu_cal_page.load(std::sync::atomic::Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            std::hint::black_box(&self.ecu_page.page)
        } else {
            self.default_page
        }
    }
}

//----------------------------------------------------------------------------------------------
// Implement DerefMut for CalSegSync
// Deref to XCP page and increment the modification counter
// This is undefined behaviour, because the reference to XCP data page will escape from its mutex
impl<T> std::ops::DerefMut for CalSeg<T>
where
    T: CalPageTrait,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        warn!("Unsafe deref mut to XCP page of {}, this is undefined behaviour !!", self.get_name());
        let mut p = self.xcp_page.lock();
        p.ctr = p.ctr.wrapping_add(1);
        let r: *mut T = &mut p.page;
        // @@@@ Usafe - For testing only
        unsafe { &mut *r }
    }
}

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
            //_not_send_sync_marker: PhantomData,
            _not_sync_marker: PhantomData,
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
// Read lock guard for calibration pages

/// Read lock guard that provides consistent read only access to a calibration page
pub struct ReadLockGuard<'a, T: CalPageTrait> {
    page: &'a T,
}

impl<T: CalPageTrait> Deref for ReadLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.page
    }
}

//----------------------------------------------------------------------------------------------
// Test
// Tests for the calibration segment
//----------------------------------------------------------------------------------------------

#[cfg(test)]
mod cal_tests {

    #![allow(dead_code)]
    use super::*;
    use crate::xcp;
    use std::sync::Arc;
    use std::thread;
    use xcp::*;
    use xcp_type_description::prelude::*;

    //-----------------------------------------------------------------------------
    // Test helpers

    fn is_copy<T: Sized + Copy>() {}
    fn is_send<T: Sized + Send>() {}
    fn is_sync<T: Sized + Sync>() {}
    fn is_clone<T: Sized + Clone>() {}
    fn is_send_clone<T: Sized + Send + Clone>() {}
    fn is_send_sync<T: Sized + Send + Sync>() {}

    /// Write to json file
    pub fn save<T, P: AsRef<std::path::Path>>(page: &T, filename: P) -> Result<(), std::io::Error>
    where
        T: serde::Serialize,
    {
        let path = filename.as_ref();
        info!("Save to file {}", path.display());
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        let s = serde_json::to_string(page).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("serde_json::to_string failed: {}", e)))?;
        std::io::Write::write_all(&mut writer, s.as_ref())?;
        Ok(())
    }

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug, Clone, Copy, XcpTypeDescription)]
    struct CalPageTest1 {
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
    }

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug, Clone, Copy, XcpTypeDescription)]
    struct CalPageTest2 {
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
    }

    fn task_calseg(cal_seg: CalSeg<CalPageTest2>) -> u32 {
        trace!("task_calseg start");
        let mut i: u32 = 0;
        loop {
            i += 1;
            thread::yield_now();
            if cal_seg.byte1 != 0 {
                break;
            }
            cal_seg.sync();
        }
        trace!("task_calseg end, loop count = {}", i);
        i
    }

    #[test]
    fn test_calibration_segment_basics() {
        //
        const CAL_PAGE_TEST1: CalPageTest1 = CalPageTest1 {
            byte1: 0,
            byte2: 0,
            byte3: 0,
            byte4: 0,
        };
        //
        const CAL_PAGE_TEST2: CalPageTest2 = CalPageTest2 {
            byte1: 0,
            byte2: 0,
            byte3: 0,
            byte4: 0,
        };

        let xcp = xcp_test::test_setup(log::LevelFilter::Info);

        // Check markers
        is_sync::<Xcp>();
        is_sync::<XcpEvent>();
        //is_sync::<DaqEvent>();
        is_copy::<CalPage1>();
        is_send::<CalPage1>();
        //is_sync::<CalPage>();
        is_send::<CalSeg<CalPage1>>();
        //is_sync::<CalSeg<CalPage1>>(); // CalSeg is not sync !
        is_clone::<CalSeg<CalPage1>>();
        //is_copy::<CalSeg<CalPage1>>(); // CalSeg is not copy

        // Interiour mutability, page switch and unwanted compiler optimizations
        let cal_page_test1 = xcp.create_calseg("CalPageTest1", &CAL_PAGE_TEST1);
        cal_page_test1.register_fields();
        let mut test = cal_page_test1.byte1;
        assert_eq!(test, 0);
        let index = cal_page_test1.get_index();
        assert_eq!(index, 0);
        // @@@@ - unsafe - Test
        unsafe {
            let data: u8 = 1;
            let offset = &CAL_PAGE_TEST1.byte1 as *const u8 as usize - &CAL_PAGE_TEST1 as *const _ as *const u8 as usize;
            assert!(offset == 0);
            cb_write(0x80000000u32, 1, &data, 0);
        }
        cal_page_test1.sync();
        test = cal_page_test1.byte1;
        assert_eq!(cal_page_test1.byte1, 1);
        assert_eq!(test, 1);
        cb_set_cal_page(1, XCP_CAL_PAGE_FLASH, CAL_PAGE_MODE_ECU | CAL_PAGE_MODE_ALL);
        test = cal_page_test1.byte1;
        assert_eq!(cal_page_test1.byte1, 0);
        assert_eq!(test, 0);
        cb_set_cal_page(1, XCP_CAL_PAGE_RAM, CAL_PAGE_MODE_ECU | CAL_PAGE_MODE_ALL);

        // Move to threads
        let cal_page_test2 = xcp.create_calseg("CalPageTest2", &CAL_PAGE_TEST2);
        cal_page_test2.register_fields();
        let index = cal_page_test2.get_index();
        assert_eq!(index, 1); // Segment index
        cal_page_test2.sync();
        assert!(cal_page_test2.byte1 == 0);
        let t1 = thread::spawn({
            let c = CalSeg::clone(&cal_page_test2);
            assert_eq!(c.get_clone_count(), 3); // 1 explicit clones, 1 for Xcp calseg_list and the original
            move || {
                task_calseg(c);
            }
        });
        let t2 = thread::spawn({
            let c = CalSeg::clone(&cal_page_test2);
            assert_eq!(c.get_clone_count(), 4); // 2 explicit clones, 1 for Xcp calseg_list and the original
            move || {
                task_calseg(c);
            }
        });
        // @@@@ - unsafe - Test
        unsafe {
            let offset = &CAL_PAGE_TEST2.byte4 as *const u8 as usize - &CAL_PAGE_TEST2 as *const _ as *const u8 as usize;
            assert!(offset == 3);
            assert!(index == 1);
            let data: u8 = 1;
            cb_write(0x80010000u32, 1, &data, 0);
            let data: u8 = 2;
            cb_write(0x80010001u32, 1, &data, 0);
            let data: u8 = 3;
            cb_write(0x80010002u32, 1, &data, 0);
            let data: u8 = 4;
            cb_write(0x80010003u32, 1, &data, 0);
        }
        t1.join().unwrap();
        t2.join().unwrap();
        cal_page_test2.sync();
        assert!(cal_page_test2.byte1 == 1);
        assert!(cal_page_test2.byte2 == 2);
        assert!(cal_page_test2.byte3 == 3);
        assert!(cal_page_test2.byte4 == 4);

        //t1.join().unwrap();
        //t2.join().unwrap();

        // Test drop and expected size
        let size = std::mem::size_of::<CalSeg<CalPageTest2>>();
        let clones = cal_page_test2.get_clone_count();
        info!("CalSeg: {} size = {} bytes, clone_count = {}", cal_page_test2.get_name(), size, clones);
        assert_eq!(size, 32);
        assert!(clones == 2); // 2 clones move to threads and dropped
    }

    // #[test]
    // fn test_calibration_segment_corner_cases() {
    //     let xcp = xcp_test::test_setup(log::LevelFilter::Info);
    // Zero size
    // #[derive(serde::Serialize, serde::Deserialize)]
    // #[derive(Debug, Clone, Copy, XcpTypeDescription)]
    // struct CalPageTest1 {}
    // const CAL_PAGE_TEST1: CalPageTest1 = CalPageTest1 {};
    // let cal_page_test1 = xcp.create_calseg("CalPageTest1", &CAL_PAGE_TEST1, false).register_fields();
    // cal_page_test1.sync();
    // drop(cal_page_test1);
    // Maximum size
    // #[derive(Debug, Clone, Copy, XcpTypeDescription)]
    // struct CalPageTest2 {
    //     a: [u8; 0x10000],
    // }
    // const CAL_PAGE_TEST2: CalPageTest2 = CalPageTest2 { a: [0; 0x10000] };
    // let cal_page_test2 = xcp.create_calseg("CalPageTest2", &CAL_PAGE_TEST2).register_fields();
    // cal_page_test2.sync();
    // drop(cal_page_test2);
    //}

    // #[test]
    // fn test_calibration_segment_performance() {
    //     let xcp = xcp_test::test_setup(log::LevelFilter::Info);

    //     const CAL_PAGE: CalPage0 = CalPage0 { stop: 0 };

    //     let mut cal_seg1 = xcp.create_calseg("calseg1", &CAL_PAGE);

    //     // Create 10 tasks with 10 clones of cal_seg1
    //     let mut t = Vec::new();
    //     let loop_count = Arc::new(parking_lot::Mutex::new(Vec::with_capacity(10)));
    //     let start = Instant::now();
    //     for i in 0..10 {
    //         let c = CalSeg::clone(&cal_seg1);
    //         trace!("task {} clone = {}", i, cal_seg1.get_clone_count());
    //         let l = loop_count.clone();
    //         t.push(thread::spawn(move || {
    //             let n = task_calseg(c);
    //             l.lock().push(n);
    //         }));
    //     }
    //     thread::sleep(Duration::from_millis(1000));
    //     cal_seg1.stop = 1; // deref_mut
    //     t.into_iter().for_each(|t| t.join().unwrap());

    //     let duration = start.elapsed().as_micros();
    //     info!("Duration: {}us", duration);
    //     let tot_loop_count: u32 = loop_count.lock().iter().sum();
    //     info!("Loop counts: tot = {}, {:.3}us per loop", tot_loop_count, duration as f64 / tot_loop_count as f64);
    // }

    //-----------------------------------------------------------------------------
    // Test file read and write of a cal_seg

    #[cfg(feature = "serde")]
    #[test]
    fn test_calibration_segment_persistence() {
        xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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
        save(&mut_page, "test_cal_seg.json").unwrap();

        // Create a cal_seg with a mut_page from file test_cal_seg.json aka CAL_PAR_RAM, and a default page from CAL_PAR_FLASH
        let cal_seg = xcp.create_calseg("test_cal_seg", &CAL_PAR_FLASH);
        cal_seg.load("test_cal_seg.json").unwrap();

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

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
    struct CalPage1 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
    struct CalPage2 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
    struct CalPage3 {
        a: u32,
        b: u32,
        c: u32,
    }

    static FLASH_PAGE1: CalPage1 = CalPage1 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE2: CalPage2 = CalPage2 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE3: CalPage3 = CalPage3 { a: 2, b: 4, c: 6 };

    #[cfg(feature = "serde")]
    macro_rules! test_is_mut {
        ( $s:ident ) => {
            if $s.a != 1 || $s.b != 3 || $s.c != 5 {
                panic!("test_is_mut: failed, s.a!=1 || s.b!=3 || s.c!=5");
            }
            trace!("test_is_mut: a={}, b={}, c={}", $s.a, $s.b, $s.c);
        };
    }

    #[cfg(feature = "serde")]
    macro_rules! test_is_default {
        ( $s:ident ) => {
            if $s.a != 2 || $s.b != 4 || $s.c != 6 {
                panic!("test_is_default: failed, s.a!=2 || s.b!=4 || s.c!=6");
            }
            trace!("test_is_default: a={}, b={}, c={}", $s.a, $s.b, $s.c);
        };
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_cal_page_switch() {
        xcp_test::test_setup(log::LevelFilter::Info);
        let xcp = Xcp::get();
        let mut_page: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
        save(&mut_page, "test1.json").unwrap();
        //save(&mut_page, "test2.json").unwrap();
        let cal_seg = xcp.create_calseg("test1", &FLASH_PAGE1);
        cal_seg.load("test1.json").unwrap();
        info!("load");
        cal_seg.sync();
        info!("sync");
        assert_eq!(
            xcp.get_ecu_cal_page(),
            XcpCalPage::Ram,
            "XCP should be on RAM page here, there is no independant page switching yet"
        );
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
        let _ = std::fs::remove_file("test1.json");
    }

    //-----------------------------------------------------------------------------
    // Test cal page freeze
    #[cfg(feature = "serde")]
    #[test]
    fn test_cal_page_freeze() {
        let xcp = xcp_test::test_setup(log::LevelFilter::Info);

        assert!(std::mem::size_of::<CalPage1>() == 12);
        assert!(std::mem::size_of::<CalPage2>() == 12);
        assert!(std::mem::size_of::<CalPage3>() == 12);

        let mut_page1: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
        save(&mut_page1, "test1.json").unwrap();

        // Create calseg1 from def
        let calseg1 = xcp.create_calseg("test1", &FLASH_PAGE1);
        calseg1.load("test1.json").unwrap();

        test_is_mut!(calseg1);

        // Freeze calseg1 to new test1.json
        let _ = std::fs::remove_file("test1.json");
        cb_freeze_cal(); // Save mut_page to file "test1.json"
        calseg1.sync();

        // Create calseg2 from freeze file test1.json of calseg1
        std::fs::copy("test1.json", "test2.json").unwrap();
        let calseg2 = xcp.create_calseg("test2", &FLASH_PAGE2);
        calseg2.load("test2.json").unwrap();

        test_is_mut!(calseg2);

        let _ = std::fs::remove_file("test1.json");
        let _ = std::fs::remove_file("test2.json");
    }

    //-----------------------------------------------------------------------------
    // Test cal page trait compiler errors

    #[test]
    fn test_cal_page_trait() {
        let xcp = xcp_test::test_setup(log::LevelFilter::Info);

        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
        struct Page1 {
            a: u32,
        }

        const PAGE1: Page1 = Page1 { a: 1 };
        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
        struct Page2 {
            b: u32,
        }

        const PAGE2: Page2 = Page2 { b: 1 };
        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
        struct Page3 {
            c: u32,
        }

        const PAGE3: Page3 = Page3 { c: 1 };

        let s1 = &xcp.create_calseg("test1", &PAGE1);
        let s2 = &xcp.create_calseg("test2", &PAGE2);
        let s3 = &xcp.create_calseg("test3", &PAGE3);

        info!("s1: {}", s1.get_name());
        info!("s2: {}", s2.get_name());
        info!("d3: {}", s3.get_name());

        let d1: Box<dyn CalSegTrait> = Box::new(s1.clone());
        info!("d1: {}", d1.get_name());
        is_send::<Box<dyn CalSegTrait + Send>>();

        #[allow(clippy::vec_init_then_push)]
        let v: Vec<Box<dyn CalSegTrait>> = vec![Box::new(s1.clone()), Box::new(s2.clone()), Box::new(s3.clone()), Box::new(s3.clone())];
        for (i, s) in v.iter().enumerate() {
            info!(" {}: {}", i, s.get_name());
        }

        let a: Arc<Mutex<Vec<Box<dyn CalSegTrait>>>> = Arc::new(Mutex::new(Vec::new()));
        {
            let mut v = a.lock();
            v.push(Box::new(s1.clone()));
            v.push(Box::new(s2.clone()));
            v.push(Box::new(s3.clone()));
            v.push(Box::new(s3.clone()));
        }

        let c = a.clone();
        let t = thread::spawn(move || {
            let v = c.lock();
            for (i, s) in v.iter().enumerate() {
                info!("thread {}: {}", i, s.get_name());
            }
        });
        t.join().unwrap();
    }
}
