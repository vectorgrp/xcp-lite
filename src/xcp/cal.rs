//-----------------------------------------------------------------------------
// Module cal
// Calibration segment descriptor

//-----------------------------------------------------------------------------
// Submodules

// Calibration segment
pub mod cal_seg;
pub use cal_seg::CalSeg;

//-----------------------------------------------------------------------------

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use parking_lot::Mutex;
use std::default;
use std::sync::Arc;

use crate::registry;

use crate::xcp;
use xcp::Xcp;
use xcp::XcpCalPage;

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
// Trait CalSegTrait

pub trait CalSegTrait
where
    Self: Send,
{
    // Get the calibration segment name
    //fn get_name(&self) -> &'static str;

    fn get_name(&self) -> &'static str {
        Xcp::get().get_calseg_name(self.get_index())
    }

    // Set the calibration segment index
    fn set_index(&mut self, index: usize);

    // Get the calibration segment index
    fn get_index(&self) -> usize;

    // Set freeze requests
    fn set_freeze_request(&self) {}

    // Set init request
    fn set_init_request(&self) {}

    /// Read from xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// dst must be valid
    // @@@@ UNSAFE function
    unsafe fn read(&self, offset: u16, len: u8, dst: *mut u8) -> bool {
        let _ = offset;
        let _ = len;
        let _ = dst;
        false
    }

    /// Write to xcp_page
    /// # Safety
    /// src must be valid
    // @@@@ UNSAFE function
    unsafe fn write(&self, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        let _ = offset;
        let _ = len;
        let _ = src;
        let _ = delay;
        false
    }

    // Flush delayed modifications
    fn flush(&self) {}
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
            calseg: Xcp::get().create_calseg(instance_name, default_page),
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

//-----------------------------------------------------------------------------
// CalSegDescriptor

pub struct CalSegDescriptor {
    name: &'static str,
    calseg: Arc<Mutex<dyn CalSegTrait>>,
    size: usize,
}

impl CalSegDescriptor {
    pub fn new(name: &'static str, calseg: Arc<Mutex<dyn CalSegTrait>>, size: usize) -> CalSegDescriptor {
        CalSegDescriptor { name, calseg, size }
    }
    pub fn get_name(&self) -> &'static str {
        self.name
    }
    pub fn get_size(&self) -> usize {
        self.size
    }
    pub fn set_init_request(&mut self) {
        self.calseg.lock().set_init_request();
    }

    pub fn set_freeze_request(&mut self) {
        self.calseg.lock().set_freeze_request();
    }
}

//-----------------------------------------------------------------------------
// CalSegList

/// Calibration segment descriptor list
/// The Xcp singleton holds this type
/// Calibration segments are created via the Xcp singleton
pub struct CalSegList(Vec<CalSegDescriptor>);

impl CalSegList {
    /// Create a calibration segment  
    /// # Panics  
    /// Panics if the calibration segment name already exists  
    /// Panics if the calibration page size exceeds 64k or is ZeroSized
    pub fn create_calseg<T>(&mut self, name: &'static str, default_page: &'static T) -> CalSeg<T>
    where
        T: CalPageTrait,
    {
        // Check size of calibration page
        assert!(std::mem::size_of::<T>() <= 0x10000 && std::mem::size_of::<T>() != 0, "CalPage size is 0 or exceeds 64k");

        // Check for duplicate name
        self.0.iter().for_each(|s| {
            assert!(s.get_name() != name, "CalSeg {} already exists", name);
        });

        // Create the calibration segment
        let index = self.0.len();
        let calseg = CalSeg::create(index, *default_page, default_page);

        // Create the calibration segment descriptor
        let c = CalSeg::clone(&calseg);
        let a: Arc<Mutex<dyn CalSegTrait>> = Arc::new(Mutex::new(c)); // Heap allocation
        let calseg_descr = CalSegDescriptor::new(name, a, std::mem::size_of::<T>());

        // Add the calibration segment descriptor to the list
        self.0.push(calseg_descr);

        info!(
            "Create CalSeg: {} index={}, sizeof<Page>={}, sizeof<CalSeg>={}",
            name,
            index,
            std::mem::size_of::<T>(),
            std::mem::size_of::<CalSeg<T>>()
        );

        calseg
    }

    pub fn get_name(&self, i: usize) -> &'static str {
        self.0[i].get_name()
    }

    pub fn get_index(&self, name: &str) -> Option<usize> {
        for (i, s) in self.0.iter().enumerate() {
            if s.get_name() == name {
                return Some(i);
            }
        }
        None
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.get_name().cmp(b.get_name()));
        self.0.iter_mut().enumerate().for_each(|(i, s)| {
            s.calseg.lock().set_index(i);
        });
    }

    pub fn register(&mut self) {
        // Sort the calibration segments by name to get a deterministic order
        self.sort_by_name();

        // Register all calibration segments in the registry
        // Address is index<<16, addr_ext is 0
        for (i, d) in self.0.iter().enumerate() {
            debug!("Register CalSeg {}, size={}", d.get_name(), d.get_size());
            assert!(i == d.calseg.lock().get_index());

            // Address calculation
            // Address format for calibration segment field is index | 0x8000 in high word, addr_ext is 0
            // (CANape does not support addr_ext in memory segments)
            let index: u16 = i.try_into().unwrap();
            let size = d.get_size().try_into().unwrap();
            let _ = registry::get_lock().as_mut().unwrap().cal_seg_list.add_cal_seg(d.get_name(), index, size);
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn set_freeze_request(&mut self) {
        self.0.iter_mut().for_each(CalSegDescriptor::set_freeze_request);
    }

    pub fn set_init_request(&mut self) {
        self.0.iter_mut().for_each(CalSegDescriptor::set_init_request);
    }

    pub fn iter(&self) -> std::slice::Iter<'_, CalSegDescriptor> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }

    /// Read from xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// Raw pointer dst must point to valid memory with len bytes size
    /// offset and len must match the size and position of the field
    /// # Panics
    /// Invalid calibration segment index
    /// offset out of calibration segment boundaries
    // @@@@ UNSAFE - direct memory access with pointer arithmetic
    pub unsafe fn read_from(&self, index: usize, offset: u16, len: u8, dst: *mut u8) -> bool {
        unsafe { self.0[index].calseg.lock().read(offset, len, dst) }
    }

    /// Write to xcp_page
    /// # Safety
    /// Raw pointer src must point to valid memory with len bytes size
    /// offset and len must match the size and position of the field
    /// # Panics
    /// Invalid calibration segment index
    /// offset out of calibration segment boundaries
    // @@@@ UNSAFE - direct memory access with pointer arithmetic
    pub unsafe fn write_to(&self, index: usize, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        unsafe { self.0[index].calseg.lock().write(offset, len, src, delay) }
    }

    // Flush delayed modifications in all calibration segments
    pub fn flush(&self) {
        self.0.iter().for_each(|s| {
            s.calseg.lock().flush();
        });
    }

    pub fn new() -> CalSegList {
        CalSegList(Vec::new())
    }
}

impl default::Default for CalSegList {
    fn default() -> Self {
        Self::new()
    }
}

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

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
    struct CalPageTest1 {
        byte1: u8,
        byte2: u8,
        byte3: u8,
        byte4: u8,
    }

    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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
        cal_page_test1.register_fields();
        let mut test = cal_page_test1.byte1;
        assert_eq!(test, 0);
        let index = cal_page_test1.get_index();
        assert_eq!(index, 0);
        // @@@@ UNSAFE - Test
        unsafe {
            let data: u8 = 1;
            let offset = &CAL_PAGE_TEST1.byte1 as *const u8 as usize - &CAL_PAGE_TEST1 as *const _ as *const u8 as usize;
            assert!(offset == 0);
            cb_write(0x80010000u32, 1, &data, 0);
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
        let cal_page_test2 = CalSeg::new("CalPageTest2", &CAL_PAGE_TEST2);
        cal_page_test2.register_fields();
        let index = cal_page_test2.get_index();
        assert_eq!(index, 1); // Segment index
        cal_page_test2.sync();
        assert!(cal_page_test2.byte1 == 0);
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
        // @@@@ UNSAFE - Test
        unsafe {
            let offset = &CAL_PAGE_TEST2.byte4 as *const u8 as usize - &CAL_PAGE_TEST2 as *const _ as *const u8 as usize;
            assert!(offset == 3);
            assert!(index == 1);
            let data: u8 = 1;
            cb_write(0x80020000u32, 1, &data, 0);
            let data: u8 = 2;
            cb_write(0x80020001u32, 1, &data, 0);
            let data: u8 = 3;
            cb_write(0x80020002u32, 1, &data, 0);
            let data: u8 = 4;
            cb_write(0x80020003u32, 1, &data, 0);
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

        info!("CalSeg: {} size = {} bytes", cal_page_test2.get_name(), size);
        assert_eq!(size, 32);
    }

    //-----------------------------------------------------------------------------
    // Test file read and write of a cal_seg

    #[test]
    fn test_calibration_segment_persistence() {
        let xcp = xcp_test::test_setup();

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

        // Create a test_cal_page.json file with values from CAL_PAR_RAM
        let mut_page: Box<CalPage> = Box::new(CAL_PAR_RAM);
        save(&mut_page, "test_cal_seg.json").unwrap();

        // Create a cal_seg with a mut_page from file test_cal_seg.json aka CAL_PAR_RAM, and a default page from CAL_PAR_FLASH
        let cal_seg = CalSeg::new("test_cal_seg", &CAL_PAR_FLASH);
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

    macro_rules! test_is_mut {
        ( $s:ident ) => {
            if $s.a != 1 || $s.b != 3 || $s.c != 5 {
                panic!("test_is_mut: failed, s.a!=1 || s.b!=3 || s.c!=5");
            }
        };
    }

    macro_rules! test_is_default {
        ( $s:ident ) => {
            if $s.a != 2 || $s.b != 4 || $s.c != 6 {
                panic!("test_is_default: failed, s.a!=2 || s.b!=4 || s.c!=6");
            }
        };
    }

    #[test]
    fn test_cal_page_switch() {
        let xcp = xcp_test::test_setup();

        let mut_page: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
        save(&mut_page, "test1.json").unwrap();
        //save(&mut_page, "test2.json").unwrap();
        let cal_seg = CalSeg::new("test1", &FLASH_PAGE1);
        cal_seg.load("test1.json").unwrap();
        info!("load");
        cal_seg.sync();
        info!("sync");
        assert_eq!(
            xcp.get_ecu_cal_page(),
            XcpCalPage::Ram,
            "XCP should be on RAM page here, there is no independent page switching yet"
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

    #[test]
    fn test_cal_page_freeze() {
        xcp_test::test_setup();

        assert!(std::mem::size_of::<CalPage1>() == 12);
        assert!(std::mem::size_of::<CalPage2>() == 12);
        assert!(std::mem::size_of::<CalPage3>() == 12);

        let mut_page1: CalPage1 = CalPage1 { a: 1, b: 3, c: 5 };
        save(&mut_page1, "test1.json").unwrap();

        // Create calseg1 from def
        let calseg1 = CalSeg::new("test1", &FLASH_PAGE1);
        calseg1.load("test1.json").unwrap();

        test_is_mut!(calseg1);

        // Freeze calseg1 to new test1.json
        let _ = std::fs::remove_file("test1.json");
        cb_freeze_cal(); // Save mut_page to file "test1.json"
        calseg1.sync();

        // Create calseg2 from freeze file test1.json of calseg1
        std::fs::copy("test1.json", "test2.json").unwrap();
        let calseg2 = CalSeg::new("test2", &FLASH_PAGE2);
        calseg2.load("test2.json").unwrap();

        test_is_mut!(calseg2);

        let _ = std::fs::remove_file("test1.json");
        let _ = std::fs::remove_file("test2.json");
    }

    //-----------------------------------------------------------------------------
    // Test cal page trait compiler errors

    #[test]
    fn test_cal_page_trait() {
        xcp_test::test_setup();

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

        let s1 = &CalSeg::new("test1", &PAGE1);
        let s2 = &CalSeg::new("test2", &PAGE2);
        let s3 = &CalSeg::new("test3", &PAGE3);

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

    //-----------------------------------------------------------------------------
    // Test cal page write and CalCell
    #[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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
        static_calseg.register_fields();

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

        value.sync(); // Sync to ECU page of this clone

        // Check value changed in the ECU page of this clone
        assert_eq!(value.test1, 2);
        assert_eq!(value.test2, -3);
        assert_eq!(value.test3, 4);
        assert_eq!(value.test4, 5.0);
        assert!(value.test5);
        assert_eq!(value.test6, 6.0);

        // Now create another clone and check the values
        let value = STATIC_CAL_SEG.get().unwrap().clone_calseg();
        assert_eq!(value.test1, 2); // Check value changed
        assert_eq!(value.test2, -3); // Read from ECU page
        assert_eq!(value.test3, 4);
        assert_eq!(value.test4, 5.0);
        assert!(value.test5);
        assert_eq!(value.test6, 6.0);
    }
}
