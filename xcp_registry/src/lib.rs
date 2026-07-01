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
    pub addr: Option<Ipv4Addr>,
    pub port: Option<u16>,
    pub baud_rate: Option<u32>,
}

impl Default for McXcpTransportLayer {
    fn default() -> Self {
        McXcpTransportLayer {
            protocol_name: "UDP",
            addr: Some(Ipv4Addr::new(127, 0, 0, 1)),
            port: Some(5555),
            baud_rate: None,
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

// Expand a typedef-typed slot (a single struct, or an array/matrix of structs) into flattened
// leaf instances. A scalar struct is expanded in place; an array of structs is unrolled element
// by element, each element getting a dotted index suffix (`name._i`, or `name._iy_ix` for a 2D
// matrix) and an address offset of `element_index * typedef.size`. The dotted `._i` form is used
// because the A2L writer sanitizes `[` / `]` to `_`.
#[allow(clippy::too_many_arguments)]
fn expand_typedef_slot(
    reg: &Registry,
    new_instances: &mut McInstanceList,
    typedef_index: &HashMap<&'static str, usize>,
    base_name: &str,
    root_instance_address: &McAddress,
    base_offset: i32,
    dim_type: &McDimType,
    typedef: &McTypeDef,
) {
    let [x_dim, y_dim] = dim_type.get_dim();

    // Scalar struct (no array dimensions): expand in place.
    if x_dim <= 1 && y_dim <= 1 {
        collect_flattened_instances(reg, new_instances, typedef_index, base_name.to_string(), root_instance_address, base_offset, typedef);
        return;
    }

    // Array or matrix of structs: unroll element by element.
    let columns = x_dim.max(1);
    let rows = y_dim.max(1);
    let stride = typedef.size as i32;
    for iy in 0..rows {
        for ix in 0..columns {
            let element_index = iy as i32 * columns as i32 + ix as i32;
            let element_offset = base_offset + element_index * stride;
            let element_name = if y_dim > 1 {
                format!("{}._{}_{}", base_name, iy, ix)
            } else {
                format!("{}._{}", base_name, ix)
            };
            collect_flattened_instances(reg, new_instances, typedef_index, element_name, root_instance_address, element_offset, typedef);
        }
    }
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
            let field_typedef = reg.typedef_list.get(i).unwrap();
            // A field may be a single struct or an array/matrix of structs; unroll arrays.
            expand_typedef_slot(
                reg,
                new_instances,
                typedef_index,
                &mangled_name,
                root_instance_address,
                root_address_offset + field.offset as i32,
                &field.dim_type,
                field_typedef,
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
            if let Some(i) = typedef_index.get(typedef_name) {
                // A top-level instance may be a single struct or an array/matrix of structs.
                expand_typedef_slot(
                    reg,
                    &mut flat_instance_list,
                    typedef_index,
                    &name,
                    instance.get_address(),
                    0,
                    instance.get_dim_type(),
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

#[cfg(test)]
mod flatten_tests {
    use super::*;

    fn offset_of(reg: &Registry, name: &str) -> i32 {
        reg.instance_list
            .get_instance(name, McObjectType::Characteristic, None)
            .unwrap_or_else(|| panic!("flattened instance '{}' not found", name))
            .get_address()
            .get_addr_offset()
    }

    // Flattening must unroll arrays of structs element by element, both as a nested
    // typedef field and as a top-level instance, accumulating per-element offsets.
    #[test]
    fn flatten_array_of_structs() {
        let mut reg = Registry::new();
        let cal = McSupportData::new(McObjectType::Characteristic);

        // typedef Inner { a: u8 @0, b: u16 @2 }  size 4
        reg.add_typedef("Inner", 4).unwrap();
        reg.add_typedef_field("Inner", "a", McDimType::new(McValueType::Ubyte, 1, 1), cal.clone(), 0).unwrap();
        reg.add_typedef_field("Inner", "b", McDimType::new(McValueType::Uword, 1, 1), cal.clone(), 2).unwrap();

        // typedef Outer { items: [Inner; 3] @0, count: u32 @12 }  size 16
        reg.add_typedef("Outer", 16).unwrap();
        reg.add_typedef_field("Outer", "items", McDimType::new(McValueType::new_typedef("Inner"), 3, 1), cal.clone(), 0)
            .unwrap();
        reg.add_typedef_field("Outer", "count", McDimType::new(McValueType::Ulong, 1, 1), cal.clone(), 12).unwrap();

        // instance outer: Outer @ calseg offset 0x10
        reg.instance_list
            .add_instance(
                "outer",
                McDimType::new(McValueType::new_typedef("Outer"), 1, 1),
                cal.clone(),
                McAddress::new_calseg_rel("seg", 0x10),
            )
            .unwrap();

        // instance arr: [Inner; 2] @ calseg offset 0x40
        reg.instance_list
            .add_instance(
                "arr",
                McDimType::new(McValueType::new_typedef("Inner"), 2, 1),
                cal.clone(),
                McAddress::new_calseg_rel("seg", 0x40),
            )
            .unwrap();

        flatten_registry(&mut reg);

        // Typedefs are dropped after flattening
        assert!(reg.typedef_list.is_empty(), "typedef list should be empty after flattening");

        // Nested array-of-structs field is unrolled with dotted indices and accumulated offsets
        assert_eq!(offset_of(&reg, "outer.items._0.a"), 0x10);
        assert_eq!(offset_of(&reg, "outer.items._0.b"), 0x12);
        assert_eq!(offset_of(&reg, "outer.items._1.a"), 0x14);
        assert_eq!(offset_of(&reg, "outer.items._1.b"), 0x16);
        assert_eq!(offset_of(&reg, "outer.items._2.a"), 0x18);
        assert_eq!(offset_of(&reg, "outer.items._2.b"), 0x1A);
        assert_eq!(offset_of(&reg, "outer.count"), 0x1C);

        // Top-level array-of-structs instance is unrolled too
        assert_eq!(offset_of(&reg, "arr._0.a"), 0x40);
        assert_eq!(offset_of(&reg, "arr._0.b"), 0x42);
        assert_eq!(offset_of(&reg, "arr._1.a"), 0x44);
        assert_eq!(offset_of(&reg, "arr._1.b"), 0x46);

        // 7 leaves from outer (3 * 2 + 1) plus 4 from arr (2 * 2)
        assert_eq!(reg.instance_list.len(), 11);
    }
}
