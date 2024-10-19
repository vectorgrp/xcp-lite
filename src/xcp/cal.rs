//-----------------------------------------------------------------------------
// Module cal
// Calibration segment descriptor

//-----------------------------------------------------------------------------
// Submodules

// Calibration segment

pub mod cal_seg;
use cal_seg::CalPageTrait;
use cal_seg::CalSeg;
use cal_seg::CalSegTrait;

//-----------------------------------------------------------------------------

use std::default;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::reg;
use crate::xcp;
use xcp::Xcp;

//-----------------------------------------------------------------------------
// Implement RegisterFields for all types that implement xcp_type_description::XcpTypeDescription

pub trait RegisterFieldsTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription,
{
    fn register_fields(&self, calseg_name: &'static str) -> &Self;
}

impl<T> RegisterFieldsTrait for T
where
    T: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription,
{
    fn register_fields(&self, calseg_name: &'static str) -> &Self {
        trace!("Register all fields in {}", calseg_name);

        for field in self.type_description().unwrap().iter() {
            let c = reg::RegistryCharacteristic::new(
                Some(calseg_name),
                field.name().to_string(),
                reg::RegistryDataType::from_rust_type(field.datatype()),
                field.comment(),
                field.min(),
                field.max(),
                field.unit(),
                if field.x_dim() == 0 { 1 } else { field.x_dim() },
                if field.y_dim() == 0 { 1 } else { field.y_dim() },
                field.offset() as u64,
            );

            Xcp::get().get_registry().lock().unwrap().add_characteristic(c).expect("Duplicate");
        }
        self
    }
}

//-----------------------------------------------------------------------------
// CalSegDescriptor

struct CalSegDescriptor {
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
        self.calseg.lock().unwrap().set_init_request();
    }

    pub fn set_freeze_request(&mut self) {
        self.calseg.lock().unwrap().set_freeze_request();
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
        let calseg = CalSeg::new(index, *default_page, default_page);

        // Create the calibration segment descriptor
        let c = CalSeg::clone(&calseg);
        let a: Arc<Mutex<dyn CalSegTrait>> = Arc::new(Mutex::new(c)); // Heap allocation
        let calseg_descr = CalSegDescriptor::new(name, a, std::mem::size_of::<T>());

        // Add the calibration segment descriptor to the list
        self.0.push(calseg_descr);

        info!(
            "Create CalSeg: {} index={}, clone_count={}, sizeof<Page>={}, sizeof<CalSeg>={}",
            name,
            index,
            calseg.get_clone_count(),
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
            let mut m = s.calseg.lock().unwrap();
            m.set_index(i);
        });
    }

    pub fn register(&mut self) {
        // Sort the calibration segments by name to get a deterministic order
        self.sort_by_name();

        // Register all calibration segments in the registry
        // Address is index<<16, addr_ext is 0
        for (i, d) in self.0.iter().enumerate() {
            trace!("Register CalSeg {}, size={}", d.get_name(), d.get_size());
            assert!(i == d.calseg.lock().unwrap().get_index());
            Xcp::get()
                .get_registry()
                .lock()
                .unwrap()
                .add_cal_seg(d.get_name(), i.try_into().unwrap(), d.get_size().try_into().unwrap());
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

    // Read from xcp_page or default_page depending on the active XCP page
    // # Safety
    // Raw pointer dst must point to valid memory with len bytes size
    // offset and len must match the size and position of the field
    // #Panics
    // Invalid calibration segment index
    // offset out of calibration segment boundaries
    // @@@@ Unsafe - direct memory access with pointer arithmetic
    pub unsafe fn read_from(&self, index: usize, offset: u16, len: u8, dst: *mut u8) -> bool {
        let m = self.0[index].calseg.lock().unwrap();
        m.read(offset, len, dst)
    }

    // Write to xcp_page
    // # Safety
    // Raw pointer src must point to valid memory with len bytes size
    // offset and len must match the size and position of the field
    // #Panics
    // Invalid calibration segment index
    // offset out of calibration segment boundaries
    // @@@@ Unsafe - direct memory access with pointer arithmetic
    pub unsafe fn write_to(&self, index: usize, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        let m = self.0[index].calseg.lock().unwrap();
        m.write(offset, len, src, delay)
    }

    // Flush delayed modifications in all calibration segments
    pub fn flush(&self) {
        self.0.iter().for_each(|s| {
            let m = s.calseg.lock().unwrap();
            m.flush();
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
