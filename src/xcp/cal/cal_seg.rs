#![allow(dead_code)]

//----------------------------------------------------------------------------------------------
// Module cal_seg
// Calibration Segment CalSeg

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use parking_lot::Mutex;

use std::{marker::PhantomData, ops::Deref, ops::DerefMut, sync::Arc};

use crate::xcp;
use xcp::Xcp;
use xcp::XcpCalPage;

use crate::registry;
use registry::RegisterFieldsTrait;

use super::CalPageTrait;
use super::CalSegTrait;

//----------------------------------------------------------------------------------------------
// Calibration parameter page wrapper for T with modification counter, init and freeze requests

#[derive(Debug, Copy, Clone)]
struct Page<T: CalPageTrait> {
    ctr: u16,
    inner: T,
}

#[derive(Debug, Copy, Clone)]
struct XcpPage<T: CalPageTrait> {
    init_request: bool,
    freeze_request: bool,
    page: Page<T>,
}

#[derive(Debug, Copy, Clone)]
struct EcuPage<T: CalPageTrait> {
    page: Page<T>,
}

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
    index: usize,
    default_page: &'static T,
    ecu_page: Box<EcuPage<T>>,
    xcp_page: Arc<Mutex<XcpPage<T>>>,
    _not_sync_marker: PhantomData<std::cell::Cell<()>>, // CalSeg is send, not sync
}

//----------------------------------------------------------------------------------------------
// CalSeg Register

// Impl register_fields for types which implement RegisterFieldsTrait
impl<T> CalSeg<T>
where
    T: CalPageTrait + RegisterFieldsTrait,
{
    /// Register all nested fields of a calibration segment as seperate instances with mangled names in the registry
    /// Requires the calibration page to implement XcpTypeDescription
    pub fn register_fields(&self) -> &Self {
        self.default_page.register_calseg_fields(self.get_name());
        self
    }
    /// Register all fields of a calibration segment in the registry using a typedef
    /// Register an instance of this typedef with instance name = type name
    /// Requires the calibration page to implement XcpTypeDescription
    /// Instancename is the typename of T
    pub fn register_typedef(&self) -> &Self {
        self.default_page.register_calseg_typedef(self.get_name());
        self
    }
}

//----------------------------------------------------------------------------------------------
// CalSeg

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Create a calibration segment for a calibration parameter struct T (calibration page type) and register it in the XCP singletons CalSeg list
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
        Xcp::get().create_calseg(instance_name, default_page)
    }

    /// Create a raw CalSeg with index
    /// Create a calibration segment for a calibration parameter struct T (called calibration page type)
    /// With a name and static const default values, which will be the "FLASH" page
    /// The mutable "RAM" page is initialized from name.json, if load_json==true and if it exists, otherwise with default
    /// CalSeg is Send and implements Clone, so clones can be safely send to other threads
    /// This comes with the cost of maintaining a shadow copy of the calibration page for each clone
    /// On calibration tool changes, sync copies the shadow (xcp_page) to the active page (ecu_page)
    ///
    /// # Arguments
    /// * `index` - Index of the calibration segment
    /// * `init_page` - Initial calibration page
    /// * `default_page` - Default calibration page
    /// # Returns
    /// A CalSeg instance
    /// # Panics
    /// If the name is not unique
    /// If the maximum number of calibration segments is reached, CANape supports a maximum of 255 calibration segments
    ///
    pub fn create(index: usize, init_page: T, default_page: &'static T) -> CalSeg<T> {
        CalSeg {
            index,
            default_page,
            ecu_page: Box::new(EcuPage {
                page: Page { ctr: 0, inner: init_page },
            }),
            xcp_page: Arc::new(Mutex::new(XcpPage {
                init_request: false,
                freeze_request: false,
                page: Page { ctr: 0, inner: init_page },
            })),
            _not_sync_marker: PhantomData,
        }
    }

    /// Get the calibration segment name
    pub fn get_name(&self) -> &'static str {
        Xcp::get().get_calseg_name(self.index)
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
            // @@@@ TODO Avoid the lock, when there is no pending modification for the XCP page
            let mut xcp_page = self.xcp_page.lock();

            // Freeze - save xcp page to json file
            // @@@@ TODO don't panic, if the file can't be written
            if xcp_page.freeze_request {
                xcp_page.freeze_request = false;
                info!("freeze: save {}.json", self.get_name());

                let mut path = std::path::PathBuf::from(self.get_name());
                path.set_extension("json");

                let file = std::fs::File::create(path).unwrap();
                let mut writer = std::io::BufWriter::new(file);
                let s = serde_json::to_string(&xcp_page.page.inner)
                    .map_err(|e| std::io::Error::other(format!("serde_json::to_string failed: {}", e)))
                    .unwrap();
                std::io::Write::write_all(&mut writer, s.as_ref()).unwrap();
            }

            // Init - copy the default calibration page back to xcp page to reset it to default values
            if xcp_page.init_request {
                xcp_page.init_request = false;
                // @@@@ UNSAFE - Implementation of init cal page in sync() with non mut self
                unsafe {
                    info!("init: {}: default_page => xcp_page ({})", self.get_name(), xcp_page.page.ctr,);

                    let src_ptr = self.default_page as *const T;
                    #[allow(clippy::ptr_cast_constness)]
                    let dst_ptr = &xcp_page.page.inner as *const _ as *mut T;
                    core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);
                }

                // Increment the modification counter to distribute the new xcp page to all clones
                xcp_page.page.ctr += 1;
            }

            // Sync - Copy shared xcp_page.page to ecu_page.page in this clone of the calibration segment
            if xcp_page.page.ctr != self.ecu_page.page.ctr {
                trace!("sync: {}: xcp_page ({}) => ecu_page ({})", self.get_name(), xcp_page.page.ctr, self.ecu_page.page.ctr);
                // @@@@ UNSAFE - Copy xcp_page to ecu_page
                unsafe {
                    let dst_ptr = &self.ecu_page.page as *const _ as *mut Page<T>; // Box<EcuPage<T>>
                    let src_ptr = &(xcp_page.page) as *const _;
                    core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);
                }
                modified = true;
            }

            modified
        }
    }
}

//----------------------------------------------------------------------------------------------
// CalSegTrait for CalSeg

impl<T> CalSegTrait for CalSeg<T>
where
    T: CalPageTrait,
{
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

    // @@@@ UNSAFE - function
    unsafe fn read(&self, offset: u16, len: u8, dst: *mut u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let xcp_page = self.xcp_page.lock();
            unsafe {
                let src: *const u8 = (&xcp_page.page as *const _ as *const u8).add(offset as usize);
                core::ptr::copy_nonoverlapping(src, dst, len as usize);
            }
            true
        } else {
            unsafe {
                let src: *const u8 = (self.default_page as *const _ as *const u8).add(offset as usize);
                core::ptr::copy_nonoverlapping(src, dst, len as usize);
            }
            true
        }
    }

    // @@@@ UNSAFE - function
    unsafe fn write(&self, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        assert!(offset as usize + len as usize <= std::mem::size_of::<T>());
        if Xcp::get().get_xcp_cal_page() == XcpCalPage::Ram {
            let mut xcp_page = self.xcp_page.lock(); // .unwrap(); // std::sync::MutexGuard
            unsafe {
                let dst: *mut u8 = (&xcp_page.page as *const _ as *mut u8).add(offset as usize);
                core::ptr::copy_nonoverlapping(src, dst, len as usize);
            }
            if delay == 0 {
                // Increment modification counter
                xcp_page.page.ctr = xcp_page.page.ctr.wrapping_add(1);
            }
            true
        } else {
            false // Write to default page is not allowed
        }
    }

    fn flush(&self) {
        let mut xcp_page = self.xcp_page.lock();
        xcp_page.page.ctr = xcp_page.page.ctr.wrapping_add(1); // Increment modification counter
    }
}

//----------------------------------------------------------------------------------------------
// Deref for CalSeg
// Used for testing only

#[cfg(test)]
impl<T> Deref for CalSeg<T>
where
    T: CalPageTrait,
{
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        if xcp::XCP.ecu_cal_page.load(std::sync::atomic::Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            std::hint::black_box(&self.ecu_page.page.inner)
        } else {
            self.default_page
        }
    }
}

#[cfg(test)]
impl<T> std::ops::DerefMut for CalSeg<T>
where
    T: CalPageTrait,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        let mut p = self.xcp_page.lock();
        p.page.ctr = p.page.ctr.wrapping_add(1);
        let r: *mut T = &mut p.page.inner;
        unsafe { &mut *r }
    }
}

//----------------------------------------------------------------------------------------------
// Clone for CalSeg

impl<T> Clone for CalSeg<T>
where
    T: CalPageTrait,
{
    fn clone(&self) -> Self {
        // Sync ECU page with XCP page before cloning the ECU page
        self.sync();

        // Clone
        CalSeg {
            index: self.index,
            default_page: self.default_page,      // &T
            ecu_page: self.ecu_page.clone(),      // Clone for each thread
            xcp_page: Arc::clone(&self.xcp_page), // Share Arc<Mutex<T>>
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
            self.xcp_page.lock().page.inner = page;
            self.xcp_page.lock().page.ctr += 1;
            self.sync();
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
        let s = serde_json::to_string(&self.xcp_page.lock().page.inner).map_err(|e| std::io::Error::other(format!("serde_json::to_string failed: {}", e)))?;
        std::io::Write::write_all(&mut writer, s.as_ref())?;
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// Read lock guard for CalSeg

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Read lock guard that provides consistent read only access to a calibration page
    /// Consistent read access to the calibration segment while the lock guard is held
    pub fn read_lock(&self) -> ReadLockGuard<'_, T> {
        self.sync();
        let xcp_or_default_page = if xcp::XCP.ecu_cal_page.load(std::sync::atomic::Ordering::Relaxed) == XcpCalPage::Ram as u8 {
            std::hint::black_box(&self.ecu_page.page.inner)
        } else {
            self.default_page
        };
        ReadLockGuard { page: xcp_or_default_page }
    }
}

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
// Write lock guard for CalSeg

/// Write lock guard that provides consistent write access to a calibration page
/// Makes the changes visible in this CalSeg after the guard is dropped, all other clones of the CalSeg will see the changes on their next sync
/// This should be used for testing only, mutable parameters are not supported yet

pub struct WriteLockGuard<'a, T: CalPageTrait> {
    lock: parking_lot::lock_api::MutexGuard<'a, parking_lot::RawMutex, XcpPage<T>>,
    calseg: &'a CalSeg<T>,
}

impl<T> CalSeg<T>
where
    T: CalPageTrait,
{
    /// Consistent write access to the calibration segments working page while the lock guard is held
    pub fn write_lock(&self) -> WriteLockGuard<'_, T> {
        let lock: parking_lot::lock_api::MutexGuard<'_, parking_lot::RawMutex, XcpPage<T>> = self.xcp_page.lock();
        WriteLockGuard { lock, calseg: self }
    }
}

impl<T: CalPageTrait> Deref for WriteLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.lock.page.inner
    }
}

impl<T: CalPageTrait> DerefMut for WriteLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lock.page.inner
    }
}

impl<T: CalPageTrait> Drop for WriteLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.page.ctr = self.lock.page.ctr.wrapping_add(1); // Increment modification counter to let all other clones know about the changes

        // Sync the changes to the ECU page of this calibration segment, all other clones will be updated on their next sync
        // Can not use CalSeg::sync() here, because it would lock the mutex again
        // @@@@ UNSAFE - Copy xcp_page to ecu_page
        unsafe {
            let dst_ptr = &self.calseg.ecu_page.page as *const _ as *mut Page<T>; // Box<EcuPage<T>>
            let src_ptr = &(self.lock.page) as *const _;
            core::ptr::copy_nonoverlapping(src_ptr, dst_ptr, 1);
        }
    }
}
