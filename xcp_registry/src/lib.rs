//-----------------------------------------------------------------------------
// Crate xcp_registry
// Registry for calibration segments, instances of parameters and measurement signals, type definitions and events
// Standalone pure-Rust crate — no C/xcplib dependency

#![crate_type = "lib"]
#![crate_name = "xcp_registry"]

// EPK calibration segment constants
// Used by the A2L reader/writer to identify the XCPlite EPK segment
pub(crate) const EPK_SEG_NAME: &str = "epk";

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
//pub use mc_address::McAddrMode;
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
                    .set_y_axis_ref(field.y_axis_ref())
                    .set_x_axis_input_quantity(field.x_axis_input_quantity())
                    .set_y_axis_input_quantity(field.y_axis_input_quantity());
                if field.is_volatile() {
                    mc_support_data = mc_support_data.set_qualifier(McObjectQualifier::Volatile);
                }
                let dim_type = McDimType::new(value_type, field.x_dim(), field.y_dim());
                let _ = get_lock()
                    .as_mut()
                    .unwrap()
                    .add_typedef_field(type_name, field.name(), dim_type, mc_support_data, field.addr_offset());
            }

            // Register the instance only if an instance name is provided
            // McAddress offset is 0 in this case, otherwise the caller is responsible to create the instance with desired offset,
            // McSupportData must contain a valid object type
            let base_addr = if let Some(event_id) = event_id {
                McAddress::new_event_dyn(0, event_id, 0)
            } else {
                McAddress::new_calseg_rel(calseg_name.unwrap(), 0)
            };
            if level == 0 {
                if let Some(instance_name) = instance_name {
                    let mc_support_data = McSupportData::new(default_object_type);
                    let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                        instance_name,
                        McDimType::new(McValueType::new_typedef(type_name), 1, 1),
                        mc_support_data,
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
                    .set_y_axis_ref(field.y_axis_ref())
                    .set_x_axis_input_quantity(field.x_axis_input_quantity())
                    .set_y_axis_input_quantity(field.y_axis_input_quantity());
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
                        McDimType::new(value_type, field.x_dim(), field.y_dim()),
                        mc_support_data,
                        McAddress::new_event_dyn(0, event_id, field.addr_offset() as i32), // @@@@ TODO: offset as i32
                    );
                }
                // Calibration segment relative addressing
                else if let Some(calseg_name) = calseg_name {
                    // Axis annotation classifier
                    if field.is_axis() {
                        mc_support_data = mc_support_data.set_object_type(McObjectType::Axis);
                        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                            field_name,
                            McDimType::new(value_type, field.x_dim(), 0),
                            mc_support_data,
                            McAddress::new_calseg_rel(calseg_name, field.addr_offset() as i32),
                        );
                    }
                    // otherwise it is always a characteristic, don't care about other classifiers yet
                    else {
                        mc_support_data = mc_support_data.set_object_type(McObjectType::Characteristic);
                        let _ = get_lock().as_mut().unwrap().instance_list.add_instance(
                            field_name,
                            McDimType::new(value_type, field.x_dim(), field.y_dim()),
                            mc_support_data,
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
/// Open for modification, mutable access through Mutex
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
/// Move registry to read only singleton
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
    if reg.get_flatten_typedefs_mode() {
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
            let _ = new_instances.add_instance(mangled_name, field.dim_type.clone(), field.mc_support_data.clone(), address);
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
                log::error!(
                    "Instance {}: Multidimensional field of type {} can not be flattened, dimension {} ignored",
                    name,
                    typedef_name,
                    instance.dim_type.get_dim()[0]
                );
                // This is not possible, we don't unroll arrays, just ignore the dimension
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
            let _ = flat_instance_list.add_instance(name, instance.dim_type.clone(), instance.get_mc_support_data().clone(), *instance.get_address());
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
