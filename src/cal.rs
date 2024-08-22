//-----------------------------------------------------------------------------
// Module cal
// Calibration segment descriptor

//-----------------------------------------------------------------------------
// Submodules

// Calibration segment
mod cal_seg;
pub use cal_seg::*;

//-----------------------------------------------------------------------------

use std::default;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use crate::xcp;
use xcp::Xcp;

#[cfg(feature = "auto_reg")]
use crate::reg;

//-----------------------------------------------------------------------------
// CalPageTrait

/// Calibration parameter page structs which can be wrapped by CalSeg, must implement this trait
/// Calibration pages must be Sized + Send + Sync + Copy + Clone + 'static
/// Optional:
/// The CalPageTrait is used to register all fields in the XCP registry
/// The CalPageTrait is used to load and save the calibration page from/to a json file
///
#[cfg(not(feature = "auto_reg"))]
#[cfg(not(feature = "json"))]
pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static,
{
}

#[cfg(feature = "auto_reg")]
#[cfg(feature = "json")]
pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription + serde::Serialize + serde::de::DeserializeOwned,
{
    fn load_from_file(name: &str) -> Result<Self, std::io::Error>;

    fn save_to_file(&self, name: &str);

    fn register_fields(&self, calseg_name: &'static str);
}

#[cfg(feature = "auto_reg")]
#[cfg(not(feature = "json"))]
pub trait CalPageTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + XcpTypeDescription,
{
    fn register_fields(&self, calseg_name: &'static str);
}

//-----------------------------------------------------------------------------
// Implement CalPageTrait for all types that may be a calibration page

#[cfg(feature = "auto_reg")]
#[cfg(feature = "json")]
impl<T> CalPageTrait for T
where
    T: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription + serde::Serialize + serde::de::DeserializeOwned,
{
    fn load_from_file(name: &str) -> Result<Self, std::io::Error> {
        trace!("Load parameter file {}", name);
        let file = std::fs::File::open(name)?;
        let reader = std::io::BufReader::new(file);
        let page = serde_json::from_reader::<_, Self>(reader)?;
        Ok(page)
    }

    fn save_to_file(&self, name: &str) {
        info!("Save parameter file {}", name);
        let file = std::fs::File::create(name).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        let s = serde_json::to_string(self).unwrap();
        std::io::Write::write_all(&mut writer, s.as_ref()).unwrap();
    }

    fn register_fields(&self, calseg_name: &'static str) {
        trace!("Register all fields in {}", calseg_name);

        for field in self.type_description().unwrap().iter() {
            let c = reg::RegistryCharacteristic::new(
                calseg_name,
                field.name().to_string(),
                reg::RegistryDataType::from_rust_type(field.datatype()),
                field.comment(),
                field.min(),
                field.max(),
                field.unit(),
                if field.x_dim() == 0 { 1 } else { field.x_dim() },
                if field.y_dim() == 0 { 1 } else { field.y_dim() },
                field.offset(),
                Xcp::XCP_ADDR_EXT_APP, // segment relative addressing
            );

            Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
        }
    }
}

#[cfg(feature = "auto_reg")]
#[cfg(not(feature = "json"))]
impl<T> CalPageTrait for T
where
    T: Sized + Send + Sync + Copy + Clone + 'static + XcpTypeDescription,
{
    fn register_fields(&self, calseg_name: &'static str) {
        trace!("Register all fields in {}", calseg_name);

        for field in self.type_description().unwrap().iter() {
            let c = reg::RegistryCharacteristic::new(
                calseg_name,
                field.name().to_string(),
                field.datatype(),
                field.comment(),
                field.min(),
                field.max(),
                field.unit(),
                if field.x_dim() == 0 { 1 } else { field.x_dim() },
                if field.y_dim() == 0 { 1 } else { field.y_dim() },
                field.offset(),
                Xcp::XCP_ADDR_EXT_APP, // segment relative addressing
            );

            Xcp::get().get_registry().lock().unwrap().add_characteristic(c);
        }
    }
}

#[cfg(not(feature = "auto_reg"))]
#[cfg(not(feature = "json"))]
impl<T> CalPageTrait for T where T: Sized + Send + Sync + Copy + Clone + 'static {}

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

#[allow(unused_variables)]
#[allow(unused_mut)]

impl CalSegList {
    /// Create a calibration segment
    /// # Panics
    /// Panics if the calibration segment name already exists
    pub fn create_calseg<T>(&mut self, name: &'static str, default_page: &'static T, auto_reg: bool, load_json: bool) -> CalSeg<T>
    where
        T: CalPageTrait,
    {
        // Check for duplicate name
        self.0.iter().for_each(|s| {
            assert!(s.get_name() != name, "CalSeg {} already exists", name);
        });

        // Register all fields
        #[cfg(feature = "auto_reg")]
        if auto_reg {
            default_page.register_fields(name);
        }
        // Load the active calibration page from file or set to default
        let mut page = *default_page;

        #[cfg(feature = "json")]
        if load_json {
            let filename = format!("{}.json", name);
            if std::path::Path::new(&filename).exists() {
                page = CalPageTrait::load_from_file(&filename).unwrap_or(*default_page);
                info!("Load parameter file {}.json as RAM page", name);
            } else {
                info!("File {}.json does not exist, using default page", name);
            }
        }

        #[cfg(not(feature = "json"))]
        if load_json {
            error!("Feature json not enabled");
        }

        // Create the calibration segment
        let index = self.0.len();
        let calseg = CalSeg::new(index, page, default_page);

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
        {
            for (i, d) in self.0.iter().enumerate() {
                trace!("Register CalSeg {}, size={}", d.get_name(), d.get_size());
                assert!(i == d.calseg.lock().unwrap().get_index());
                Xcp::get()
                    .get_registry()
                    .lock()
                    .unwrap()
                    .add_cal_seg(d.get_name(), Xcp::get_calseg_addr_base(i), Xcp::XCP_ADDR_EXT_APP, d.get_size() as u32);
            }
        }
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn set_freeze_request(&mut self) {
        self.0.iter_mut().for_each(|s| s.set_freeze_request());
    }

    pub fn set_init_request(&mut self) {
        self.0.iter_mut().for_each(|s| s.set_init_request());
    }

    /// Read from xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// Memory access is unsafe, src checked to be inside a calibration segment
    pub fn read_from(&self, index: usize, offset: u16, len: u8, dst: *mut u8) -> bool {
        let m = self.0[index].calseg.lock().unwrap();
        // @@@@ unsafe - Call to unsafe method, read from xcp_page or default_page depending on the active XCP page
        unsafe { m.read(offset, len, dst) }
    }

    /// Read from xcp_page or default_page depending on the active XCP page
    /// # Safety
    /// Memory access is unsafe, dst checked to be inside a calibration segment
    pub fn write_to(&self, index: usize, offset: u16, len: u8, src: *const u8, delay: u8) -> bool {
        let m = self.0[index].calseg.lock().unwrap();
        // @@@@ unsafe - Call to unsafe method, read from xcp_page or default_page depending on the active XCP page
        unsafe { m.write(offset, len, src, delay) }
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
