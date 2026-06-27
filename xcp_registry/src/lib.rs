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

// McRegisterType (runtime support for the #[derive(McRegisterType)] proc-macro)
mod mc_register_type;
pub use mc_register_type::McRegisterContext;
pub use mc_register_type::McRegisterTarget;
pub use mc_register_type::McRegisterType;

// Re-export the derive macro (serde-style: one import brings the trait and the derive)
pub use xcp_register_type_derive::McRegisterType;

//-----------------------------------------------------------------------------
// Removed: the old RegisterFieldsTrait blanket impl that walked StructDescriptor /
// FieldDescriptor trees. Registration is now generated directly by #[derive(McRegisterType)]
// (see mc_register_type.rs and the xcp_register_type_derive crate).

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

// @@@@ TODO: Check if this AI induced change from OnceLock to Mutex is what we desired ??????
/// Closed registry singleton
/// (Finalized and read only after call to Registry::close())
static CLOSED_REGISTRY: Mutex<Option<&'static Registry>> = Mutex::new(None);
//static CLOSED_REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();

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
    if CLOSED_REGISTRY.lock().is_some() {
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
    if CLOSED_REGISTRY.lock().is_none() {
        // Close the mutable registry singleton
        // Flatten typedefs
        log::warn!("Automatically close registry - call to Registry::get() without explictic close!");
        close();
    }
    CLOSED_REGISTRY.lock().unwrap()
}

/// Get closed status   
pub fn is_closed() -> bool {
    CLOSED_REGISTRY.lock().is_some()
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
    *CLOSED_REGISTRY.lock() = Some(Box::leak(Box::new(reg)));
    // CLOSED_REGISTRY.get().unwrap()

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

#[doc(hidden)]
pub mod registry_test {
    use super::*;

    pub fn test_reinit() {
        *REGISTRY.lock() = Some(Registry::new());
        *CLOSED_REGISTRY.lock() = None;
    }
}
