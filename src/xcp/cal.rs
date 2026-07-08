//-----------------------------------------------------------------------------
// Module cal
// Calibration segment descriptor

//-----------------------------------------------------------------------------

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use std::default;
use std::sync::Arc;

#[cfg(feature = "linkme")]
use linkme::distributed_slice;
#[cfg(feature = "linkme")]
use std::sync::Once;
#[cfg(feature = "linkme")]
use std::sync::atomic::{AtomicU16, Ordering};

use crate::registry;

use crate::xcp;
use crate::xcp::xcplib;
use xcp::Xcp;

use std::{marker::PhantomData, ops::Deref, ops::DerefMut};

use registry::{McRegisterTarget, McRegisterType};

//-----------------------------------------------------------------------------
// CalPageTrait

// Calibration pages must be Sized + Send + Sync + Copy + Clone + 'static

pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + serde::Serialize + serde::de::DeserializeOwned,
{
}

// Implement CalPageTrait for all types that may be a calibration page
impl<T> CalPageTrait for T where T: Sized + Send + Sync + Copy + Clone + 'static + serde::Serialize + serde::de::DeserializeOwned {}

//----------------------------------------------------------------------------------------------
// CalSeg

/// Thread safe calibration parameter page wrapper with interior mutability by XCP
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
    index: xcplib::tXcpCalSegIndex,
    default_page: &'static T,
    _not_sync_marker: PhantomData<std::cell::Cell<()>>, // CalSeg is send, not sync
}

//----------------------------------------------------------------------------------------------
// CalSeg Register

// Impl register for types which implement McRegisterType
impl<T> CalSeg<T>
where
    T: CalPageTrait + McRegisterType,
{
    /// Register the calibration segment in the registry as a typedef plus one top-level instance.
    /// Nested structs become nested typedefs; arrays of nested structs become dimensioned typedef
    /// instances. The instance name is the calibration segment name.
    /// Requires the calibration page to implement McRegisterType.
    ///
    /// Flattening for legacy tools that do not support typedefs is a separate, export-time
    /// transform on the registry (not a registration mode).
    pub fn register(&self) -> &Self {
        self.default_page.mc_register(McRegisterTarget::CalSeg(self.get_name()), Some(self.get_name()));
        self
    }
}

//----------------------------------------------------------------------------------------------
// CalSeg

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Create a calibration segment for a calibration parameter struct T (calibration page type)
    /// With a name and static const default values, which will be the "FLASH" page
    /// The mutable "RAM" page is initialized from name.json, if load_json==true and if it exists, otherwise with default
    /// CalSeg is Send and implements Clone, so clones can be safely send to other threads
    /// This comes with the cost of maintaining a shadow copy of the calibration page for each clone
    /// On calibration tool changes, sync copies the shadow (xcp_page) to the active page (ecu_page)
    ///
    /// # Arguments
    /// * `instance_name` - Name of the calibration segment instance
    /// * `default_page` - Default calibration page
    /// # Returns
    /// A CalSeg instance
    /// # Panics
    /// If the name is not unique
    /// If the maximum number of calibration segments is reached, CANape supports a maximum of 255 calibration segments
    pub fn new(instance_name: &'static str, default_page: &'static T) -> CalSeg<T> {
        // Create a calibration segment in the xcplib C library
        unsafe {
            let c_name = std::ffi::CString::new(instance_name).unwrap();
            let c_default_page = default_page as *const T as *const std::os::raw::c_void;
            let index = xcplib::XcpCreateCalSeg(
                c_name.as_ptr(),
                c_default_page,
                u16::try_from(std::mem::size_of::<T>()).expect("CalSeg size exceeds u16::MAX"),
            );

            if index == u16::MAX {
                panic!("xcplib_create_calseg failed for instance_name={}", instance_name);
            }
            CalSeg::<T> {
                index,
                default_page,
                _not_sync_marker: PhantomData,
            }
        }
    }

    /// Get the calibration segment name
    pub fn get_name(&self) -> &'static str {
        unsafe {
            let c_str = xcplib::XcpGetCalSegName(self.index);
            std::ffi::CStr::from_ptr(c_str).to_str().unwrap()
        }
    }

    /// Get the calibration segment index
    pub fn get_index(&self) -> usize {
        self.index as usize
    }

    /// Construct a `CalSeg` from a link-time registered descriptor (used by the [`cal_seg!`] macro).
    ///
    /// On the first call this triggers deterministic creation of *all* calibration segments whose
    /// descriptors were collected into [`CAL_SEG_REGISTRY`] at link time: they are sorted by name
    /// and created in that order, so the segment index (A2L MEMORY_SEGMENT number) is stable across
    /// runs regardless of creation order or threads. Subsequent calls only read the resolved index.
    #[cfg(feature = "linkme")]
    #[doc(hidden)]
    pub fn __from_registry(default_page: &'static T, descriptor: &'static CalSegDescriptor) -> CalSeg<T> {
        ensure_calsegs_created();
        let index = descriptor.index.load(Ordering::Acquire);
        assert!(
            index != XCP_UNDEFINED_CALSEG,
            "calibration segment '{}' was not registered - is XCP initialized (Xcp::init) before creating calibration segments?",
            descriptor.name
        );
        CalSeg {
            index,
            default_page,
            _not_sync_marker: PhantomData,
        }
    }
}

//----------------------------------------------------------------------------------------------
// Link-time calibration segment registration (cal_seg! macro)

/// Sentinel for an unresolved / invalid calibration segment index (matches XCP_UNDEFINED_CALSEG).
#[cfg(feature = "linkme")]
const XCP_UNDEFINED_CALSEG: u16 = 0xFFFF;

/// Descriptor for a calibration segment, collected into [`CAL_SEG_REGISTRY`] at link time by the
/// [`cal_seg!`] macro. Mirrors the C `tXcpCalSegDescriptor` placed in the `xcp_cals` ELF section.
///
/// Not part of the public API; only constructed by the `cal_seg!` macro.
#[cfg(feature = "linkme")]
#[doc(hidden)]
pub struct CalSegDescriptor {
    name: &'static str,
    default_page: *const core::ffi::c_void,
    size: u16,
    index: AtomicU16,
}

#[cfg(feature = "linkme")]
impl CalSegDescriptor {
    /// Const constructor so the descriptor can be a `static` initializer in the macro expansion.
    #[doc(hidden)]
    pub const fn new(name: &'static str, default_page: *const core::ffi::c_void, size: u16) -> Self {
        Self {
            name,
            default_page,
            size,
            index: AtomicU16::new(XCP_UNDEFINED_CALSEG),
        }
    }
}

// SAFETY: `default_page` points to an immutable, 'static calibration page. The raw pointer is only
// ever read (handed once to XcpCreateCalSeg); `index` is atomic. The descriptor is never mutated
// through shared references other than the atomic store, so it is safe to share across threads.
#[cfg(feature = "linkme")]
unsafe impl Sync for CalSegDescriptor {}

/// Distributed slice of all calibration segment descriptors created via [`cal_seg!`].
/// The linker gathers every descriptor into a contiguous section, independent of which code paths
/// actually execute, exactly like the C `xcp_cals` section consumed by `XcpInit`.
#[cfg(feature = "linkme")]
#[distributed_slice]
#[doc(hidden)]
pub static CAL_SEG_REGISTRY: [CalSegDescriptor];

/// Guards one-time creation of all registered calibration segments.
#[cfg(feature = "linkme")]
static CAL_SEG_REGISTRY_INIT: Once = Once::new();

/// Create all link-time registered calibration segments once, in name-sorted order.
#[cfg(feature = "linkme")]
fn ensure_calsegs_created() {
    CAL_SEG_REGISTRY_INIT.call_once(|| {
        // Sort by name -> deterministic, source-order- and thread-independent segment indices.
        let mut descriptors: Vec<&'static CalSegDescriptor> = CAL_SEG_REGISTRY.iter().collect();
        descriptors.sort_by(|a, b| a.name.cmp(b.name));
        for d in descriptors {
            let c_name = std::ffi::CString::new(d.name).expect("calibration segment name must not contain NUL");
            // @@@@ UNSAFE - C library call; default_page is a 'static page of d.size bytes
            let index = unsafe { xcplib::XcpCreateCalSeg(c_name.as_ptr(), d.default_page, d.size) };
            assert!(index != XCP_UNDEFINED_CALSEG, "XcpCreateCalSeg failed for calibration segment '{}'", d.name);
            d.index.store(index, Ordering::Release);
            debug!("Registered calibration segment '{}' with index {}", d.name, index);
        }
    });
}

/// Create a calibration segment.
///
/// With the **`linkme` feature enabled (default)** the segment descriptor is collected into a
/// distributed slice at link time. On first use all descriptors are sorted by name and created in
/// that order, so the segment index (A2L MEMORY_SEGMENT number) stays stable across runs regardless
/// of creation order or threads - preventing unnecessary A2L churn and avoiding any creation race.
///
/// With the **`linkme` feature disabled** this falls back to eager creation in call order (exactly
/// like [`CalSeg::new`]). This is appropriate when all calibration segments are created in a single,
/// deterministic, race-free order and you prefer not to depend on `linkme`.
///
/// # Arguments
/// * `$name` - `&'static str` name of the calibration segment instance (must be unique).
/// * `$default` - `&'static` reference to a `const`/`static` default calibration page.
///
/// # Example
/// ```ignore
/// let calseg = cal_seg!("my_params", &PARAMS);
/// calseg.register();
/// ```
#[cfg(feature = "linkme")]
#[macro_export]
macro_rules! cal_seg {
    ($name:expr, $default:expr $(,)?) => {{
        #[$crate::_private::distributed_slice($crate::_private::CAL_SEG_REGISTRY)]
        static CAL_SEG_DESCRIPTOR: $crate::_private::CalSegDescriptor =
            $crate::_private::CalSegDescriptor::new($name, $default as *const _ as *const ::core::ffi::c_void, ::core::mem::size_of_val($default) as u16);
        $crate::CalSeg::__from_registry($default, &CAL_SEG_DESCRIPTOR)
    }};
}

/// Create a calibration segment.
///
/// Fallback definition used when the **`linkme` feature is disabled**: the segment is created
/// eagerly in call order, identical to [`CalSeg::new`]. Use this when all calibration segments are
/// created in a single, deterministic, race-free order so that the assigned indices are stable.
/// Enable the `linkme` feature for link-time, name-sorted, race-free index assignment.
#[cfg(not(feature = "linkme"))]
#[macro_export]
macro_rules! cal_seg {
    ($name:expr, $default:expr $(,)?) => {{ $crate::CalSeg::new($name, $default) }};
}

//----------------------------------------------------------------------------------------------
// Clone for CalSeg

impl<T> Clone for CalSeg<T>
where
    T: CalPageTrait,
{
    fn clone(&self) -> Self {
        // Clone
        CalSeg {
            index: self.index,
            default_page: self.default_page, // &T

            _not_sync_marker: PhantomData,
        }
    }
}

//----------------------------------------------------------------------------------------------
// Load/Save for CalSeg

// Impl load and save for type which implement serde::Serialize and serde::de::DeserializeOwned
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
            *self.write_lock() = page;
            Ok(())
        } else {
            warn!("File not found: {}", path.display());
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("File not found: {}", path.display())))
        }
    }

    /// Write a calibration segment to json file
    /// Requires the calibration page type to implement serde::Serialize + serde::de::DeserializeOwned
    pub fn save<P: AsRef<std::path::Path>>(&self, filename: P) -> Result<(), std::io::Error> {
        let path = filename.as_ref();
        info!("Save {} to file {}", self.get_name(), path.display());
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        let page = self.read_lock();
        let s = serde_json::to_string(&*page).map_err(|e| std::io::Error::other(format!("serde_json::to_string failed: {}", e)))?;
        std::io::Write::write_all(&mut writer, s.as_ref())?;
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// Read lock guard for CalSeg

pub struct ReadLockGuard<'a, T: CalPageTrait> {
    page: &'a T,
    index: xcplib::tXcpCalSegIndex,
}

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Read lock guard that provides consistent read only access to a calibration page
    /// Consistent read access to the calibration segment while the lock guard is held
    pub fn read_lock(&self) -> ReadLockGuard<'_, T> {
        // Lock the calibration segment in the xcplib C library
        unsafe {
            let ptr: *const T = xcplib::XcpLockCalSeg(self.index) as *const T;
            ReadLockGuard { page: &*ptr, index: self.index }
        }
    }
}

impl<T: CalPageTrait> Drop for ReadLockGuard<'_, T> {
    fn drop(&mut self) {
        // Unlock the calibration segment in the xcplib C library
        unsafe {
            xcplib::XcpUnlockCalSeg(self.index);
        }
    }
}

impl<T: CalPageTrait> Deref for ReadLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.page
    }
}

//----------------------------------------------------------------------------------------------
// Write lock guard for CalSeg

/// Write lock guard that provides consistent write access to a calibration page
/// Makes the changes visible in this CalSeg after the guard is dropped, all other clones of the CalSeg will see the changes on their next sync
/// This should be used for testing only, mutable parameters are not supported yet
pub struct WriteLockGuard<'a, T: CalPageTrait> {
    page: &'a mut T,
    index: xcplib::tXcpCalSegIndex,
}

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Consistent write access to the calibration segments working page while the lock guard is held
    pub fn write_lock(&self) -> WriteLockGuard<'_, T> {
        unsafe {
            let ptr: *mut T = xcplib::XcpLockCalSeg(self.index) as *mut T;
            WriteLockGuard {
                page: &mut *ptr,
                index: self.index,
            }
        }
    }
}

impl<T: CalPageTrait> Deref for WriteLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.page
    }
}

impl<T: CalPageTrait> DerefMut for WriteLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.page
    }
}

impl<T: CalPageTrait> Drop for WriteLockGuard<'_, T> {
    fn drop(&mut self) {
        unsafe {
            xcplib::XcpUnlockCalSeg(self.index);
        }
    }
}

//----------------------------------------------------------------------------------------------
// CalCell

/// Cell for a CalSeg
/// Helps to create static instances of CalSeg using something like lazy_static or once_cell
/// CalCell is Sync, because the only way to access the inner CalSeg is via a clone of the CalSeg

#[derive(Debug)]
pub struct CalCell<T>
where
    T: CalPageTrait,
{
    calseg: CalSeg<T>,
}

impl<T> CalCell<T>
where
    T: CalPageTrait,
{
    /// Create a CalCell from instance_name and a default calibration page
    ///
    /// # Arguments
    /// * `instance_name` - Name of the calibration segment instance
    /// * `default_page` - Default calibration page
    /// # Returns
    /// A CalCell instance
    /// # Panics
    /// If the instance name is not unique
    /// If the maximum number of calibration segments is reached, CANape supports a maximum of 255 calibration segments
    ///
    pub fn new(instance_name: &'static str, default_page: &'static T) -> CalCell<T> {
        CalCell {
            calseg: CalSeg::new(instance_name, default_page),
        }
    }

    /// Get a clone of the calibration segment from the CalCell
    pub fn clone_calseg(&self) -> CalSeg<T> {
        self.calseg.clone()
    }
}

// Implement Sync for CalCell
// #safety
// CalCell is Send, because CalSeg is Send
// CalCell is Sync, because CalCells only public method is clone_calseg which returns a CalSeg clone, CalSeg clones are Send, not Sync
// @@@@ UNSAFE - implement Sync for CalCell
unsafe impl<T> Sync for CalCell<T> where T: CalPageTrait {}

//----------------------------------------------------------------------------------------------
// Test
// Calibration Tests
//----------------------------------------------------------------------------------------------

#[cfg(test)]
mod cal_tests {

    #![allow(dead_code)]
    use std::sync::Arc;
    use std::thread;

    use super::*;
    use crate::xcp::*;
    use registry::McRegisterType;

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

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
    struct CalPageTest1 {
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
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
            {
                let p = cal_seg.read_lock();
                if p.byte1 == 1 && p.byte2 == 2 && p.byte3 == 3 && p.byte4 == 4 {
                    break;
                }
            }
        }
        trace!("task_calseg end, loop count = {}", i);
        i
    }

    // TODO fix test_calibration_segment_basics on windows and enable it again
    #[cfg(not(target_os = "windows"))]
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

        xcp_test::test_setup();

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

        // Interior mutability, page switch and unwanted compiler optimizations
        let cal_page_test1 = CalSeg::new("CalPageTest1", &CAL_PAGE_TEST1);
        cal_page_test1.register();
        let p = cal_page_test1.read_lock();
        let mut test = p.byte1;
        drop(p);
        assert_eq!(test, 0);
        let index = cal_page_test1.get_index();
        assert_eq!(index, 1); // Index 0 is reserved for epk segment

        let mut p = cal_page_test1.write_lock();
        p.byte1 = 1;
        drop(p);

        test = cal_page_test1.read_lock().byte1;
        assert_eq!(test, 1);

        // cb_set_cal_page(1, XCP_CAL_PAGE_FLASH, CAL_PAGE_MODE_ECU | CAL_PAGE_MODE_ALL);
        // test = cal_page_test1.byte1;
        // assert_eq!(cal_page_test1.byte1, 0);
        // assert_eq!(test, 0);
        // cb_set_cal_page(1, XCP_CAL_PAGE_RAM, CAL_PAGE_MODE_ECU | CAL_PAGE_MODE_ALL);

        // Move to threads
        let cal_page_test2 = CalSeg::new("CalPageTest2", &CAL_PAGE_TEST2);
        cal_page_test2.register();
        let index = cal_page_test2.get_index();
        assert_eq!(index, 2); // Segment index

        assert!(cal_page_test2.read_lock().byte1 == 0);
        let t1 = thread::spawn({
            let c = CalSeg::clone(&cal_page_test2);
            move || {
                task_calseg(c);
            }
        });
        let t2 = thread::spawn({
            let c = CalSeg::clone(&cal_page_test2);
            move || {
                task_calseg(c);
            }
        });

        cal_page_test2.write_lock().byte1 = 1;
        cal_page_test2.write_lock().byte2 = 2;
        cal_page_test2.write_lock().byte3 = 3;
        cal_page_test2.write_lock().byte4 = 4;

        t1.join().unwrap();
        t2.join().unwrap();

        assert!(cal_page_test2.read_lock().byte1 == 1);
        assert!(cal_page_test2.read_lock().byte2 == 2);
        assert!(cal_page_test2.read_lock().byte3 == 3);
        assert!(cal_page_test2.read_lock().byte4 == 4);

        let size = std::mem::size_of::<CalSeg<CalPageTest2>>();
        info!("CalSeg: {} size = {} bytes", cal_page_test2.get_name(), size);
        assert_eq!(size, 16);
    }

    //-----------------------------------------------------------------------------
    // Test file read and write of a cal_seg

    #[test]
    fn test_calibration_segment_persistence() {
        let _xcp = xcp_test::test_setup();

        #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
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

        // Create a test_cal_page.json file with values from CAL_PAR_RAM
        let mut_page: Box<CalPage> = Box::new(CAL_PAR_RAM);
        save(&mut_page, "test_cal_seg.json").unwrap();

        // Create a cal_seg with a mut_page from file test_cal_seg.json aka CAL_PAR_RAM, and a default page from CAL_PAR_FLASH
        let cal_seg = CalSeg::new("test_cal_seg", &CAL_PAR_FLASH);
        cal_seg.load("test_cal_seg.json").unwrap();

        let cal_seg1 = cal_seg.clone();
        let cal_seg2 = cal_seg.clone();

        assert_eq!(cal_seg.read_lock().test_byte, 0x55, "test_byte != 0x55, default is RAM");
        let r = &cal_seg.read_lock().test_byte;
        assert_eq!(*r, 0x55, "&test_byte != 0x55, default is RAM");

        //xcp.set_ecu_cal_page(XcpCalPage::Flash);
        // assert_eq!(cal_seg.read_lock().test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        // assert_eq!(cal_seg1.read_lock().test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        // assert_eq!(cal_seg2.read_lock().test_byte, 0xAA, "test_byte != 0xAA from FLASH");
        // assert_eq!(*r, 0x55, "&test_byte != 0x55, reference to RAM"); // @@@@ Note: References are legal, not affected by switch

        //xcp.set_ecu_cal_page(XcpCalPage::Ram);
        assert_eq!(cal_seg.read_lock().test_byte, 0x55, "test_byte != 0x55 from RAM");
        assert_eq!(cal_seg1.read_lock().test_byte, 0x55, "test_byte != 0x55 from RAM");
        assert_eq!(cal_seg2.read_lock().test_byte, 0x55, "test_byte != 0x55 from RAM");
        drop(cal_seg2);
        drop(cal_seg1);
        let _ = cal_seg;

        std::fs::remove_file("test_cal_seg.json").ok();
    }

    //-----------------------------------------------------------------------------
    // Test cal page switching

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, McRegisterType)]
    struct CalPage1 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, McRegisterType)]
    struct CalPage2 {
        a: u32,
        b: u32,
        c: u32,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, McRegisterType)]
    struct CalPage3 {
        a: u32,
        b: u32,
        c: u32,
    }

    static FLASH_PAGE1: CalPage1 = CalPage1 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE2: CalPage2 = CalPage2 { a: 2, b: 4, c: 6 };
    static FLASH_PAGE3: CalPage3 = CalPage3 { a: 2, b: 4, c: 6 };

    // macro_rules! test_is_mut {
    //     ( $s:ident ) => {
    //         if $s.a != 1 || $s.b != 3 || $s.c != 5 {
    //             panic!("test_is_mut: failed, s.a!=1 || s.b!=3 || s.c!=5");
    //         }
    //     };
    // }

    // macro_rules! test_is_default {
    //     ( $s:ident ) => {
    //         if $s.a != 2 || $s.b != 4 || $s.c != 6 {
    //             panic!("test_is_default: failed, s.a!=2 || s.b!=4 || s.c!=6");
    //         }
    //     };
    // }

    // #[test]
    // fn test_cal_page_switch() {
    //     let xcp = xcp_test::test_setup();

    //     let mut_page: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
    //     save(&mut_page, "test1.json").unwrap();
    //     //save(&mut_page, "test2.json").unwrap();
    //     let cal_seg = CalSeg::new("test1", &FLASH_PAGE1);
    //     cal_seg.load("test1.json").unwrap();
    //     info!("load");
    //     cal_seg.sync();
    //     info!("sync");
    //     assert_eq!(
    //         xcp.get_ecu_cal_page(),
    //         XcpCalPage::Ram,
    //         "XCP should be on RAM page here, there is no independent page switching yet"
    //     );
    //     test_is_mut!(cal_seg); // Default page must be mut_page
    //     xcp.set_ecu_cal_page(XcpCalPage::Flash); // Simulate a set cal page to default from XCP master
    //     cal_seg.sync();
    //     test_is_default!(cal_seg);
    //     xcp.set_ecu_cal_page(XcpCalPage::Ram); // Switch back to ram
    //     cal_seg.sync();
    //     test_is_mut!(cal_seg);
    //     // Check if cal page switching works in a loop where the compiler might optimize the cal_page values
    //     for i in 0..10 {
    //         if i <= 50 {
    //             if cal_seg.a != 1 {
    //                 unreachable!();
    //             };
    //         } else if cal_seg.a != 2 {
    //             unreachable!();
    //         }
    //         if i == 50 {
    //             xcp.set_ecu_cal_page(XcpCalPage::Flash); // Switch to default
    //             cal_seg.sync();
    //         }
    //     }
    //     let _ = std::fs::remove_file("test1.json");
    // }

    //-----------------------------------------------------------------------------
    // Test cal page freeze

    // #[test]
    // fn test_cal_page_freeze() {
    //     xcp_test::test_setup();

    //     assert!(std::mem::size_of::<CalPage1>() == 12);
    //     assert!(std::mem::size_of::<CalPage2>() == 12);
    //     assert!(std::mem::size_of::<CalPage3>() == 12);

    //     let mut_page1: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
    //     save(&mut_page1, "test1.json").unwrap();

    //     // Create calseg1 from def
    //     let calseg1 = CalSeg::new("test1", &FLASH_PAGE1);
    //     calseg1.load("test1.json").unwrap();

    //     test_is_mut!(calseg1);

    //     // Freeze calseg1 to new test1.json
    //     let _ = std::fs::remove_file("test1.json");
    //     cb_freeze_cal(); // Save mut_page to file "test1.json"
    //     calseg1.sync();

    //     // Create calseg2 from freeze file test1.json of calseg1
    //     std::fs::copy("test1.json", "test2.json").unwrap();
    //     let calseg2 = CalSeg::new("test2", &FLASH_PAGE2);
    //     calseg2.load("test2.json").unwrap();

    //     test_is_mut!(calseg2);

    //     let _ = std::fs::remove_file("test1.json");
    //     let _ = std::fs::remove_file("test2.json");
    // }

    //-----------------------------------------------------------------------------
    // Test cal page write and CalCell
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
    struct StaticCalPage {
        test1: u8,
        test2: i64,
        test3: u32,
        test4: f32,
        test5: bool,
        test6: f64,
        test7: [u16; 2],
        test8: [[u16; 2]; 2],
    }

    // Default values for the calibration parameters
    const STATIC_CAL_PAGE: StaticCalPage = StaticCalPage {
        test1: 1,
        test2: -2,
        test3: 3,
        test4: 0.32,
        test5: false,
        test6: 0.64,
        test7: [1, 2],
        test8: [[1, 2], [3, 4]],
    };

    static STATIC_CAL_SEG: std::sync::OnceLock<CalCell<StaticCalPage>> = std::sync::OnceLock::new();

    #[test]
    fn test_calpage_write() {
        let static_calseg = STATIC_CAL_SEG.get_or_init(|| CalCell::new("static_calseg", &STATIC_CAL_PAGE)).clone_calseg();
        static_calseg.register();

        let value = STATIC_CAL_SEG.get().unwrap().clone_calseg();
        {
            let mut value = value.write_lock();
            value.test1 = 2; // Write to XCP page
            value.test2 = -3;
            value.test3 = 4;
            value.test4 = 5.0;
            value.test5 = true;
            value.test6 = 6.0;
        }

        // Check value changed in the ECU page of this clone
        assert_eq!(value.read_lock().test1, 2);
        assert_eq!(value.read_lock().test2, -3);
        assert_eq!(value.read_lock().test3, 4);
        assert_eq!(value.read_lock().test4, 5.0);
        assert!(value.read_lock().test5);
        assert_eq!(value.read_lock().test6, 6.0);

        // Now create another clone and check the values
        let value = STATIC_CAL_SEG.get().unwrap().clone_calseg();
        assert_eq!(value.read_lock().test1, 2); // Check value changed
        assert_eq!(value.read_lock().test2, -3); // Read from ECU page
        assert_eq!(value.read_lock().test3, 4);
        assert_eq!(value.read_lock().test4, 5.0);
        assert!(value.read_lock().test5);
        assert_eq!(value.read_lock().test6, 6.0);
    }
}
