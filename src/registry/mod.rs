//-----------------------------------------------------------------------------
// Module registry
// Registry for calibration segments, instances of parameters and measurement signals, type definitions and events
// Used by the register_xxx macros

use parking_lot::Mutex;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use thiserror::Error;

// Registry
mod mc_registry;
pub use mc_registry::McApplication;
pub use mc_registry::Registry;

// A2L reader and writer
mod a2l;

// McEvent
mod mc_event;
pub use mc_event::McEvent;
pub use mc_event::McEventList;
pub use mc_event::McEventListIterator;

// McCalibrationSegment
mod mc_calseg;
pub use mc_calseg::McCalibrationSegment;
pub use mc_calseg::McCalibrationSegmentList;
pub use mc_calseg::McCalibrationSegmentListIterator;

// McInstance
mod mc_instance;
pub use mc_instance::McInstance;
pub use mc_instance::McInstanceList;
pub use mc_instance::McInstanceListIterator;

// McTypeDef
mod mc_typedef;
pub use mc_typedef::McTypeDef;
pub use mc_typedef::McTypeDefField;
pub use mc_typedef::McTypeDefList;
pub use mc_typedef::McTypeDefListIterator;

// McObjectType, McSupportData
mod mc_support;
pub use mc_support::McObjectQualifier;
pub use mc_support::McObjectType;
pub use mc_support::McSupportData;

// McValueType, McDimType
mod mc_type;
pub use mc_type::McDimType;
pub use mc_type::McValueType;
pub use mc_type::McValueTypeTrait;

// McAddress
mod mc_address;
pub use mc_address::McAddress;

// McText
mod mc_text;
pub use mc_text::McIdentifier;
pub use mc_text::McText;

//-----------------------------------------------------------------------------
// Register an instance of a measurement struct or a calibration segment

#[doc(hidden)]
pub trait RegisterFieldsTrait
where
    Self: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription,
{
    fn register_calseg_fields(&self, calseg_name: &'static str);
    fn register_calseg_typedef(&self, calseg_name: &'static str);
    fn register_struct_fields(&self, instance_name: Option<&'static str>, event_id: u16);
    fn register_struct_typedef(&self, instance_name: Option<&'static str>, event_id: u16);
    fn register_as(
        struct_descriptor: &xcp_type_description::StructDescriptor,
        instance_name: Option<&'static str>,
        calseg_name: Option<&'static str>,
        event_id: Option<u16>,
        as_typedef: bool,
        level: usize,
    );
}

// Implement RegisterFieldsTrait for all types that implement xcp_type_description::XcpTypeDescription
// Note: 2 calibration segment instances from the same struct are not supported, this will create a name conflict in the registry, because type name is used as instance name to inhibit this

impl<T> RegisterFieldsTrait for T
where
    T: Sized + Send + Sync + Copy + Clone + 'static + xcp_type_description::XcpTypeDescription,
{
    // Register StructDescriptors
    // When flatten==false, fields may have inner StructDescriptors

    fn register_calseg_fields(&self, calseg_name: &'static str) {
        let type_description = self.type_description(true).unwrap();
        Self::register_as(&type_description, Some(calseg_name), Some(calseg_name), None, false, 0);
    }

    fn register_calseg_typedef(&self, calseg_name: &'static str) {
        let type_description = self.type_description(false).unwrap();
        Self::register_as(&type_description, Some(calseg_name), Some(calseg_name), None, true, 0);
    }

    fn register_struct_fields(&self, instance_name: Option<&'static str>, event_id: u16) {
        let type_description = self.type_description(true).unwrap();
        Self::register_as(&type_description, instance_name, None, Some(event_id), false, 0);
    }

    fn register_struct_typedef(&self, instance_name: Option<&'static str>, event_id: u16) {
        let type_description: xcp_type_description::StructDescriptor = self.type_description(false).unwrap();
        Self::register_as(&type_description, instance_name, None, Some(event_id), true, 0);
    }

    // @@@@ TODO Error handling
    fn register_as(
        type_description: &xcp_type_description::StructDescriptor,
        instance_name: Option<&'static str>,
        calseg_name: Option<&'static str>,
        event_id: Option<u16>,
        as_typedef: bool,
        level: usize,
    ) {
        assert!(calseg_name.is_some() || event_id.is_some(), "No calseg_name or event_id given to register_as");

        let type_name = type_description.name(); // name of the struct
        let default_object_type = if event_id.is_some() { McObjectType::Measurement } else { McObjectType::Characteristic };

        log::debug!(
            "{}: Register all fields for instance_name={:?} type_name={} calseg={:?} event={:?}, as_typedef={}",
            level,
            instance_name,
            type_description.name(),
            calseg_name,
            event_id,
            as_typedef
        );

        //--------------------------------------------------------------------------------
        // Register an instance with a typedef
        if as_typedef {
            //
            // Create a typedef struct
            let _ = get_lock().as_mut().unwrap().add_typedef(type_name, type_description.size());

            // Add all fields to the typedef
            for field in type_description.iter() {
                // Recursion, if there are nested struct_descriptors
                if let Some(struct_descriptor) = field.struct_descriptor() {
                    Self::register_as(struct_descriptor, instance_name, calseg_name, event_id, true, level + 1);
                }

                // Register a typedef component
                let value_type = McValueType::from_rust_type(field.value_type()); // In case of a nested StructDescriptor in the field, this is a Instance(type_name)
                let mut mc_support_data = McSupportData::new(get_object_type(field, default_object_type))
                    .set_comment(field.comment())
                    .set_min(field.min())
                    .set_max(field.max())
                    .set_step(field.step())
                    // .set_unit(field.unit()) // not needed if set_linear is used
                    .set_linear(field.factor(), field.offset(), field.unit())
                    .set_x_axis_ref(field.x_axis_ref())
                    .set_y_axis_ref(field.y_axis_ref());
                if field.is_volatile() {
                    mc_support_data = mc_support_data.set_qualifier(McObjectQualifier::Volatile);
                }
                let dim_type = McDimType::new(value_type, field.x_dim(), field.y_dim(), mc_support_data);
                let _ = get_lock().as_mut().unwrap().add_typedef_component(type_name, field.name(), dim_type, field.addr_offset());
            }

            // Register the instance only if an instance name is provided
            // McAddress offset is 0 in this case, other wise the caller is responsible to create the instance with desired offset,
            // McSupportData must contain a valid object type
            let base_addr = if let Some(event_id) = event_id {
                McAddress::new_event_rel(event_id, 0)
            } else {
                McAddress::new_calseg_rel(calseg_name.unwrap(), 0)
            };
            if level == 0 {
                if let Some(instance_name) = instance_name {
                    let mc_support_data = McSupportData::new(default_object_type);
                    let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                        instance_name,
                        McDimType::new(McValueType::new_typedef(type_name), 1, 1, mc_support_data),
                        base_addr,
                    );
                }
            }
        }
        //--------------------------------------------------------------------------------
        // Register instances of all fields
        else {
            // Register all fields as instances
            for field in type_description.iter() {
                assert!(field.struct_descriptor().is_none()); // This should already be flat

                // Create a field instance name
                // Prefix the field name with instance name, if there is one
                let field_name = if let Some(instance_name) = instance_name {
                    format!("{}.{}", instance_name, field.name())
                } else {
                    field.name().to_string()
                };

                // Create a McSupportData for the field
                let mut mc_support_data = McSupportData::new(McObjectType::Unspecified)
                    .set_comment(field.comment())
                    .set_min(field.min())
                    .set_max(field.max())
                    .set_step(field.step())
                    // .set_unit(field.unit()) // not needed if set_linear is used
                    .set_linear(field.factor(), field.offset(), field.unit())
                    .set_x_axis_ref(field.x_axis_ref())
                    .set_y_axis_ref(field.y_axis_ref());
                if field.is_volatile() {
                    mc_support_data = mc_support_data.set_qualifier(McObjectQualifier::Volatile);
                }
                // Get value type, may not be a type name
                let value_type = McValueType::from_rust_type(field.value_type());
                assert!(value_type != McValueType::Unknown);

                // Measurement event relative addressing
                if let Some(event_id) = event_id {
                    mc_support_data = mc_support_data.set_object_type(McObjectType::Measurement);
                    let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                        field_name,
                        McDimType::new(value_type, field.x_dim(), field.y_dim(), mc_support_data),
                        McAddress::new_event_rel(event_id, field.addr_offset() as i32),
                    );
                }
                // Calibration segment relative addressing
                else if let Some(calseg_name) = calseg_name {
                    // Axis annotation classifier
                    if field.is_axis() {
                        mc_support_data = mc_support_data.set_object_type(McObjectType::Axis);
                        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                            field_name,
                            McDimType::new(value_type, field.x_dim(), 0, mc_support_data),
                            McAddress::new_calseg_rel(calseg_name, field.addr_offset() as i32),
                        );
                    }
                    // otherwise it is always a characteristic, don't care about other classifiers yet
                    else {
                        mc_support_data = mc_support_data.set_object_type(McObjectType::Characteristic);
                        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                            field_name,
                            McDimType::new(value_type, field.x_dim(), field.y_dim(), mc_support_data),
                            McAddress::new_calseg_rel(calseg_name, field.addr_offset() as i32),
                        );
                    }
                }
            }
        }
    }
}

// helper to create McObjectType if field attribute is set
fn get_object_type(field: &xcp_type_description::FieldDescriptor, default_object_type: McObjectType) -> McObjectType {
    if field.is_axis() {
        McObjectType::Axis
    } else if field.is_characteristic() {
        McObjectType::Characteristic
    } else if field.is_measurement() {
        McObjectType::Measurement
    } else {
        default_object_type
    }
}

//----------------------------------------------------------------------------------------------
// Error

/// Error type returned by functions of the registry API
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("registry error: duplicate symbol `{0}` ")]
    Duplicate(String),

    #[error("registry error: `{0}` not found")]
    NotFound(String),

    #[error("registry error: event id not defined `{0}` ")]
    UnknownEventChannel(u16),

    #[error("unknown error")]
    Unknown,
}

//-------------------------------------------------------------------------------------------------
// McXcpTransportLayer
// XCP Transport layer parameters
// For A2l XCP IF_DATA

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct McXcpTransportLayer {
    pub protocol_name: &'static str,
    pub addr: Ipv4Addr,
    pub port: u16,
}

impl Default for McXcpTransportLayer {
    fn default() -> Self {
        McXcpTransportLayer {
            protocol_name: "UDP",
            addr: Ipv4Addr::new(127, 0, 0, 1),
            port: 5555,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Registry singleton

/// Open registry singleton
/// Open for modification, mutable access throung Mutex
/// Registry is closed when None
static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

/// Closed registry OnceLock singleton
/// (Finalized and read only after call to Registry::close())
static CLOSED_REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();

//---------------------------------------------------------------------------------------------------------
// Associated function for the registry singleton

/// Initialize the mutable registry singleton
pub fn init() {
    let mut l = REGISTRY.lock();
    if l.is_none() {
        *l = Some(Registry::new());
        log::info!("Registry initialized");
    } else {
        log::info!("Registry already initialized");
    }
}

/// Get a lock on the mutable registry singleton
/// # Panics
/// If the registry is not initialized with registry::init
/// # Returns
/// None if the registry is closed
pub fn get_lock() -> parking_lot::lock_api::MutexGuard<'static, parking_lot::RawMutex, Option<Registry>> {
    // Check if registry is closed, it should be None then
    #[cfg(not(test))]
    if CLOSED_REGISTRY.get().is_some() {
        let l = REGISTRY.lock();
        assert!(l.is_none());
        return l;
    }

    let l = REGISTRY.lock();
    assert!(l.is_some(), "Registry not initialized!");
    l
}

/// Get a reference to the closed registry singleton
/// Close and flatten if not already closed
pub fn get() -> &'static Registry {
    if CLOSED_REGISTRY.get().is_none() {
        // Close the mutable registry singleton
        // Flatten typedefs
        log::warn!("Automatically close registry - call to Registry::get() without explictic close!");
        close();
    }
    CLOSED_REGISTRY.get().unwrap()
}

/// Get closed status   
pub fn is_closed() -> bool {
    CLOSED_REGISTRY.get().is_some()
}

/// Close registry
/// No more changes allowed
/// Move registry to read only singletom
pub fn close() {
    // Check if registry is already closed
    if is_closed() {
        log::warn!("Mutable registry singleton already closed !");
        return;
    }

    // Move mutable registry singleton out
    log::info!("Mutable registry singleton closed !");
    let mut reg = get_lock().take().unwrap();

    // Flatten the registry if requested
    if reg.flatten_typedefs {
        flatten_registry(&mut reg);
    }

    // Move mutable registry singleton to closed singleton
    CLOSED_REGISTRY.get_or_init(|| reg);

    // log::info!("Registry instance list:");
    // for i in &get().instance_list {
    //     log::info!("  {}: {:?} {}", i.name, i.dim_type.value_type, i.address.get_addr_offset());
    // }
}

// Recursive helper function to build additional flattened instances from typedefs
// Collect all typedef tree leafs and mangle the instance name
fn collect_flattened_instances(
    reg: &Registry,
    new_instances: &mut McInstanceList,
    typedef_index: &HashMap<&'static str, usize>,
    name: String,
    root_instance_address: &McAddress,
    root_address_offset: i32,
    typedef: &McTypeDef,
) {
    for field in &typedef.fields {
        let mangled_name = format!("{}.{}", name, field.name);
        if let Some(typedef_name) = field.get_typedef_name() {
            let i = *typedef_index.get(typedef_name).unwrap();
            let typedef = reg.typedef_list.get(i).unwrap();
            collect_flattened_instances(
                reg,
                new_instances,
                typedef_index,
                mangled_name,
                root_instance_address,
                root_address_offset + field.offset as i32,
                typedef,
            );
        } else {
            let mut address = *root_instance_address;
            address.add_addr_offset(root_address_offset + field.offset as i32);
            let _ = new_instances.add_instance(mangled_name, field.dim_type.clone(), address);
        }
    }
}

// Collect all instance leafs and create new instances with mangled names
fn create_flattened_instance_list(reg: &mut Registry, typedef_index: &HashMap<&'static str, usize>) -> McInstanceList {
    let mut flat_instance_list = McInstanceList::new();
    for instance in &reg.instance_list {
        let name: String = instance.get_name().to_string();
        if let Some(typedef_name) = instance.get_typedef_name() {
            if instance.dim_type.get_dim()[0] > 1 {
                // Multidimensional typedef field, not supported
                log::error!("Instance {}: Multidimension field of type {} can not be flattened", name, typedef_name);
                assert!(false); // Only basic types are supported
            }

            if let Some(i) = typedef_index.get(typedef_name) {
                collect_flattened_instances(
                    reg,
                    &mut flat_instance_list,
                    typedef_index,
                    name,
                    instance.get_address(),
                    0,
                    reg.typedef_list.get(*i).unwrap(),
                );
            } else {
                log::error!("Typedef {} not found in typedef list", typedef_name);
            }
        } else {
            // No typedef, just add the instance
            let _ = flat_instance_list.add_instance(name, instance.dim_type.clone(), *instance.get_address());
        }
    }
    flat_instance_list
}

pub fn flatten_registry(reg: &mut Registry) {
    log::info!("Flattening typedef structure in registry into mangled instance names !");

    // Build typedef (name,index) hashmap
    let typedef_index: &HashMap<&str, usize> = &reg
        .typedef_list
        .into_iter()
        .enumerate()
        .map(|(index, typedef)| (typedef.get_name(), index))
        .collect::<std::collections::HashMap<_, _>>();
    // log::info!("Registry typedef index:");
    // log::Info!("{:#?}", typedef_index);

    reg.instance_list = create_flattened_instance_list(reg, typedef_index);
    reg.typedef_list.clear();
    // for i in &reg.instance_list {
    //     log::info!("  + {}: {:?} {}", i.name, i.dim_type.value_type, i.address.get_addr_offset());
    // }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
pub mod registry_test {

    use std::fs::File;
    use std::io::{self, BufRead};
    use std::net::Ipv4Addr;

    use super::*;

    use crate::CalSeg;
    use crate::registry;
    use crate::xcp::*;
    use xcp_type_description::prelude::*;

    // Reinitialize the registry singleton for testing
    pub fn test_reinit() {
        log::info!("Test setup: Clear registry singletons, reinit XCP singleton");
        {
            registry::init();
            let mut l = registry::get_lock();
            l.replace(Registry::new());
            l.as_mut().unwrap().set_app_info("test_setup", "created by test_setup", 0);
            l.as_mut().unwrap().set_app_version("test_setup", Xcp::XCP_EPK_ADDR);
        }
        // Drop the closed registry singleton (unsafe)
        #[allow(invalid_reference_casting)]
        unsafe {
            let p: *mut std::sync::OnceLock<Registry> = &registry::CLOSED_REGISTRY as *const _ as *mut _;
            let r: &mut std::sync::OnceLock<Registry> = &mut *p;
            let _reg = r.take();
        }
    }

    // Compare two files line by line
    fn compare_files(file1: &str, file2: &str) -> io::Result<()> {
        let file1 = File::open(file1)?;
        let file2 = File::open(file2)?;

        let reader1 = io::BufReader::new(file1);
        let reader2 = io::BufReader::new(file2);

        let mut differences_found: u32 = 0;

        for (line_num, (line1, line2)) in reader1.lines().zip(reader2.lines()).enumerate() {
            let line1 = line1?;
            let line2 = line2?;

            if line1 != line2 {
                log::debug!("Difference at line {}:\nFile1: {}\nFile2: {}", line_num + 1, line1, line2);
                differences_found += 1;
                break;
            }
        }

        if differences_found == 0 {
            println!("The files are identical.");
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Files are different"))
        }
    }

    //-----------------------------------------------------------------------------
    // Test registry and A2L writer
    #[test]
    fn test_registry_1() {
        // McValueType
        let i: i8 = 0;
        let t = i.get_type();
        assert_eq!(t, McValueType::Sbyte);

        // Registry
        let mut reg = Registry::new();
        reg.set_app_info("test_registry_1", "created by test_registry_1", 0);
        reg.set_app_version("EPK1.0.0", 0x80000000);
        reg.set_xcp_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);

        reg.cal_seg_list.add_cal_seg("test_cal_seg_1", 0, 4).unwrap();
        reg.cal_seg_list.add_cal_seg("test_cal_seg_2", 1, 4).unwrap();

        let mc_support_data = McSupportData::new(McObjectType::Characteristic)
            .set_max(Some(127.0 * 2.0 + 1.0))
            .set_linear(2.0, 1.0, "Deg")
            .set_comment("comment");
        reg.instance_list
            .add_instance(
                "test_characteristic_1",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_calseg_rel("test_cal_seg_1", 0),
            )
            .unwrap();

        let mc_support_data = McSupportData::new(McObjectType::Characteristic)
            .set_min(Some(-128.0 * 2.0 + 1.0))
            .set_max(Some(127.0 * 2.0 + 1.0))
            .set_linear(2.0, 1.0, "Deg")
            .set_comment("comment");
        reg.instance_list
            .add_instance(
                "test_characteristic_2",
                McDimType::new(McValueType::Sbyte, 1, 1, mc_support_data),
                McAddress::new_calseg_rel("test_cal_seg_2", 0),
            )
            .unwrap();

        let event1 = XcpEvent::new(0, 1);
        reg.event_list.add_event(McEvent::new("event1", event1.get_index(), event1.get_id(), 0)).unwrap();

        let mc_support_data = McSupportData::new(McObjectType::Measurement).set_comment("comment");
        reg.instance_list
            .add_instance(
                "test_measurement",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_event_rel(event1.get_id(), 8),
            )
            .unwrap();

        // Write A2L file and check syntax
        reg.write_a2l(&"test_registry_1.a2l", true).unwrap();
    }

    //-----------------------------------------------------------------------------
    // Test A2L writer

    #[test]
    fn test_registry_2() {
        #[derive(serde::Serialize, serde::Deserialize, XcpTypeDescription, Debug, Clone, Copy)]
        struct CalPage {
            #[characteristic(comment = "comment")]
            #[characteristic(unit = "unit")]
            #[characteristic(min = "-128.0")]
            #[characteristic(max = "127.0")]
            test_characteristic_1: i8,
            #[characteristic(comment = "comment")]
            #[characteristic(unit = "unit")]
            #[characteristic(min = "-128.0")]
            #[characteristic(max = "127.0")]
            test_characteristic_2: i8,
            test_characteristic_3: i16,
        }

        const CAL_PAGE: CalPage = CalPage {
            test_characteristic_1: 0,
            test_characteristic_2: 0,
            test_characteristic_3: 0,
        };

        let xcp = xcp_test::test_setup();

        registry::get_lock().as_mut().unwrap().set_app_info("test_registry_2", "created by test_registry_2", 0);
        registry::get_lock().as_mut().unwrap().set_app_version("EPK2.0.0", 0x80000000);
        registry::get_lock().as_mut().unwrap().set_xcp_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);

        let _ = CalSeg::new("test_cal_seg_1", &CAL_PAGE).register_fields();

        let event1_1 = xcp.create_event_ext("event1", true);
        let event1_2 = xcp.create_event_ext("event1", true);
        let event2 = xcp.create_event_ext("event2", false);

        // Check if event index is correctly handled, measurement names may not be unique
        let mc_support_data = McSupportData::new(McObjectType::Measurement).set_comment("instance 1 of test_measurement_1");
        registry::get_lock()
            .as_mut()
            .unwrap()
            .instance_list
            .add_instance(
                "test_measurement_1",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_event_rel(event1_1.get_id(), 0),
            )
            .unwrap();

        let mc_support_data = McSupportData::new(McObjectType::Measurement).set_comment("instance 2 of test_measurement_1");
        registry::get_lock()
            .as_mut()
            .unwrap()
            .instance_list
            .add_instance(
                "test_measurement_1",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_event_rel(event1_2.get_id(), 0), // Event instance 2
            )
            .unwrap();

        let mc_support_data = McSupportData::new(McObjectType::Measurement);
        registry::get_lock()
            .as_mut()
            .unwrap()
            .instance_list
            .add_instance(
                "test_measurement_2",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_event_rel(event2.get_id(), 0),
            )
            .unwrap();

        // Close registry, write A2L file and check syntax
        xcp.finalize_registry().unwrap();

        // Write JSON
        registry::get().write_json(&"test_registry_2.json").unwrap();

        // Read and compare JSON
        let mut reg2 = Registry::new();
        reg2.load_json(&"test_registry_2.json").unwrap();
        reg2.write_json(&"test_registry_2_reloaded.json").unwrap();
        compare_files("test_registry_2.json", "test_registry_2_reloaded.json").unwrap();
    }

    //-----------------------------------------------------------------------------
    // Test attribute macros
    #[ignore = "can not reopen registry singleton for other tests"]
    #[test]
    fn test_attribute_macros() {
        let xcp = xcp_test::test_setup();

        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, XcpTypeDescription)]
        struct CalPage {
            #[characteristic(comment = "Comment")]
            #[characteristic(unit = "Unit")]
            #[characteristic(min = "0")]
            #[characteristic(max = "100")]
            a: u32,
            b: u32,
            #[axis(comment = "Axis")]
            axis: [f64; 16], // This will be a AXIS type (1 dimension)

            #[characteristic(comment = "Curve")]
            curve: [f64; 16], // This will be a AXIS type (1 dimension)

            #[characteristic(comment = "Map")]
            map: [[u8; 9]; 8], // This will be a MAP type (2 dimensions)
        }
        const CAL_PAGE: CalPage = CalPage {
            a: 1,
            b: 2,
            axis: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
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

        let calseg = CalSeg::new("calseg", &CAL_PAGE);
        calseg.register_fields();
        assert_eq!(calseg.get_name(), "calseg");
        xcp.finalize_registry().unwrap();

        let reg = registry::get();
        let c = reg.instance_list.find_instance("calseg.a", McObjectType::Characteristic, None).unwrap();
        assert_eq!(c.get_dim_type().get_comment(), "Comment");
        assert_eq!(c.get_dim_type().get_unit(), "Unit");
        assert_eq!(c.get_dim_type().get_min(), Some(0.0));
        assert_eq!(c.get_dim_type().get_max(), Some(100.0));
        assert!(c.get_dim_type().get_dim()[0] <= 1);
        assert!(c.get_dim_type().get_dim()[1] <= 1);
        assert_eq!(c.address.get_addr_offset(), 328);
        assert_eq!(c.dim_type.value_type, McValueType::Ulong);

        let c = reg.instance_list.find_instance("calseg.b", McObjectType::Characteristic, None).unwrap();
        assert_eq!(c.address.get_addr_offset(), 332);

        let c = reg.instance_list.find_instance("calseg.axis", McObjectType::Axis, None).unwrap();
        assert_eq!(c.address.get_addr_offset(), 0);
        assert_eq!(c.dim_type.get_dim()[0], 16);

        let c = reg.instance_list.find_instance("calseg.curve", McObjectType::Characteristic, None).unwrap();
        assert_eq!(c.address.get_addr_offset(), 128);
        assert_eq!(c.dim_type.get_dim()[0], 16);
        assert!(c.dim_type.get_dim()[1] <= 1);

        let c = reg.instance_list.find_instance("calseg.map", McObjectType::Characteristic, None).unwrap();
        assert_eq!(c.address.get_addr_offset(), 256);
        assert_eq!(c.dim_type.get_dim()[0], 9);
        assert_eq!(c.dim_type.get_dim()[1], 8);
    }

    //-----------------------------------------------------------------------------
    // Test API
    #[test]
    fn test_registry_api() {
        let _ = xcp_test::test_setup();

        let mut reg = Registry::new();

        // Application name and version
        reg.set_app_info("test_registry_api", "created by test_registry_api", 0);
        reg.set_app_version("V1.0.0", 0);

        // Calibration segment
        reg.cal_seg_list.add_cal_seg("calseg_1", 0, 4).unwrap();
        reg.cal_seg_list.add_cal_seg("calseg_2", 1, 16).unwrap();

        // Event
        let xcp_event_1: XcpEvent = XcpEvent::new(0, 0);
        reg.event_list
            .add_event(McEvent::new("event_1", xcp_event_1.get_index(), xcp_event_1.get_id(), 100))
            .unwrap();
        let xcp_event_2: XcpEvent = XcpEvent::new(1, 0);
        reg.event_list
            .add_event(McEvent::new("event_2", xcp_event_2.get_index(), xcp_event_2.get_id(), 1000))
            .unwrap();

        // Measurement (McObjectType Measurement)
        let mc_support_data = McSupportData::new(McObjectType::Measurement).set_comment("Measurement value u8");
        reg.instance_list
            .add_instance(
                "mea_u8",
                McDimType::new(McValueType::Ubyte, 1, 1, mc_support_data),
                McAddress::new_event_rel(xcp_event_1.get_id(), 0),
            )
            .unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Measurement).set_comment("Measurement value f64");
        reg.instance_list
            .add_instance(
                "mea_f64",
                McDimType::new(McValueType::Float32Ieee, 1, 1, mc_support_data),
                McAddress::new_event_rel(xcp_event_2.get_id(), 0),
            )
            .unwrap();

        // TypeDef characteristic struct
        let t = reg.add_typedef("typedef_characteristic_1", 8).unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Characteristic);
        t.add_field(
            "field1_typedef_characteristic_2",
            McDimType::new(McValueType::TypeDef("typedef_characteristic_2".into()), 1, 1, mc_support_data),
            0,
        )
        .unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Characteristic);
        t.add_field("field2_f64", McDimType::new(McValueType::Float64Ieee, 1, 1, mc_support_data), 0).unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Axis);
        t.add_field("field3_axis_8_f64", McDimType::new(McValueType::Float64Ieee, 8, 0, mc_support_data), 0)
            .unwrap();

        let t = reg.add_typedef("typedef_characteristic_2", 8).unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Characteristic);

        t.add_field("field1_i8", McDimType::new(McValueType::Sbyte, 1, 1, mc_support_data), 0).unwrap();

        // Characteristic (McObjectType Characteristic and Axis)
        let mc_support_data = McSupportData::new(McObjectType::Characteristic)
            .set_max(Some(127.0))
            .set_comment("Characteristic value in calseg_1 with type typedef_characteristic_1");
        reg.instance_list
            .add_instance(
                "characteristic_1",
                McDimType::new(McValueType::new_typedef("typedef_characteristic_1"), 4, 2, mc_support_data),
                McAddress::new_calseg_rel("calseg_1", 0),
            )
            .unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Characteristic)
            .set_max(Some(127.0))
            .set_comment("Characteristic value in calseg_1 with type typedef_characteristic_2");
        reg.instance_list
            .add_instance(
                "characteristic_2",
                McDimType::new(McValueType::new_typedef("typedef_characteristic_2"), 4, 2, mc_support_data),
                McAddress::new_calseg_rel("calseg_2", 0),
            )
            .unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Axis).set_comment("Axis value").set_max(Some(8.0));
        reg.instance_list
            .add_instance(
                "axis_1",
                McDimType::new(McValueType::Float32Ieee, 8, 0, mc_support_data),
                McAddress::new_calseg_rel("calseg_1", 0),
            )
            .unwrap();
        let mc_support_data = McSupportData::new(McObjectType::Characteristic)
            .set_min(Some(0.0))
            .set_max(Some(1.0))
            .set_step(Some(0.05))
            .set_comment("Characteristic curve 8.1 in calseg_1 with axis_1")
            .set_x_axis_ref(Some("axis_1"));
        reg.instance_list
            .add_instance(
                "characteristic_3",
                McDimType::new(McValueType::Slonglong, 8, 1, mc_support_data),
                McAddress::new_calseg_rel("calseg_1", 0),
            )
            .unwrap();

        // Write A2L file and check syntax
        {
            reg.write_a2l(&"test_registry_api.a2l", false).unwrap();

            // Load the A2L file into another registry
            let mut reg2 = Registry::new();
            let res = reg2.load_a2l(&"test_registry_api.a2l".to_owned(), true, true, false, false);
            match res {
                Ok(warnings) => {
                    println!("A2L file reloaded, {} warnings", warnings);
                }
                Err(e) => {
                    println!("Error: {}", e);
                    panic!("Error loading A2L file");
                }
            }

            // Compare
            //assert_eq!(reg, reg2);
        }
    }

    //-----------------------------------------------------------------------------
    // Test A2L reader

    #[test]
    fn test_registry_load_a2l() {
        let _ = xcp_test::test_setup();

        // Load A2L file from main.rs: xcp_lite.a2l
        log::info!("Load A2L file xcp_lite.a2l");
        let mut reg = Registry::new();

        let res = reg.load_a2l(&"xcp_lite.a2l", true, true, true, false);
        match res {
            Ok(_) => {
                println!("A2L file loaded");
            }
            Err(e) => {
                log::error!("Error: {}", e);
                panic!("Error loading A2L file");
            }
        }

        // Write JSON file
        log::info!("Write JSON file test_registry_load_a2l.json");
        reg.write_json(&"test_registry_load_a2l.json").unwrap();

        // Write A2L file and check syntax
        log::info!("Write A2L file test_registry_load_a2l.a2l");
        reg.write_a2l(&"test_registry_load_a2l.a2l", true).unwrap();

        // Compare xcp_lite.a2l and xcp_lite2.a2l
        // let file1 = "xcp_lite.a2l";
        // let file2 = "test_registry_load_a2l.a2l";
        // if let Err(e) = compare_files(file1, file2) {
        //     log::error!("Error comparing files: {}", e);
        // }
    }
}
