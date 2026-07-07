// Module mc_register_type
// Runtime support for the `#[derive(McRegisterType)]` proc-macro (crate xcp_register_type_derive).
//
// The derive generates an `impl McRegisterType for T` whose `register` method calls the
// registry API directly (add_typedef / add_typedef_field / add_instance). No intermediate
// StructDescriptor/FieldDescriptor data structures are produced.
//
// The trait and the context type are internal (`#[doc(hidden)]`). End users never construct a
// context or call `register` directly; they use the ergonomic wrappers on `CalSeg` and the
// `daq_register_struct!` macro, which build the context internally via the provided methods.

use super::McAddress;
use super::McObjectType;
use super::McValueType;

//----------------------------------------------------------------------------------------------
// McRegisterTarget

/// Where (and in which address mode) a type is registered.
/// Internal type used by the generated code; not part of the stable public API.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum McRegisterTarget {
    /// Calibration-segment-relative addressing.
    CalSeg(&'static str),
    /// Event(measurement)-relative addressing (event id).
    Event(u16),
}

impl McRegisterTarget {
    /// Default object type when a field carries no classifier attribute.
    #[doc(hidden)]
    pub fn default_object_type(&self) -> McObjectType {
        match self {
            McRegisterTarget::Event(_) => McObjectType::Measurement,
            McRegisterTarget::CalSeg(_) => McObjectType::Characteristic,
        }
    }

    /// Build a base address for the given accumulated offset.
    #[doc(hidden)]
    pub fn address(&self, addr_offset: i32) -> McAddress {
        match self {
            McRegisterTarget::Event(event_id) => McAddress::new_event_dyn(0, *event_id, addr_offset),
            McRegisterTarget::CalSeg(name) => McAddress::new_calseg_rel(*name, addr_offset),
        }
    }
}

//----------------------------------------------------------------------------------------------
// McRegisterContext

/// Carries the registration target, the accumulated name prefix and address offset (used during
/// recursion into nested typedefs) and the nesting level.
///
/// There is no runtime mode flag: the generated code always builds typedefs. Flattening for
/// legacy tools is a separate, export-time transform on the populated registry, not a codegen
/// mode (see `xcp_register_type_derive/DESIGN.md`).
/// Internal type used by the generated code; not part of the stable public API.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct McRegisterContext {
    pub target: McRegisterTarget,
    pub instance_name: Option<&'static str>,
    pub name_prefix: String,
    pub addr_offset: u16,
    pub level: usize,
}

impl McRegisterContext {
    /// Child context used to create a nested typedef (no instance, deeper level).
    #[doc(hidden)]
    pub fn child_typedef(&self) -> McRegisterContext {
        McRegisterContext {
            target: self.target,
            instance_name: None,
            name_prefix: String::new(),
            addr_offset: 0,
            level: self.level + 1,
        }
    }
}

//----------------------------------------------------------------------------------------------
// McRegisterType

/// Implemented by `#[derive(McRegisterType)]`.
/// Internal trait: the `register` method is called only by the wrappers below.
#[doc(hidden)]
pub trait McRegisterType {
    /// Register this type into the open registry singleton according to `ctx`.
    fn register(ctx: &McRegisterContext);

    /// The registry type name (the struct identifier).
    fn mc_type_name() -> &'static str;

    /// Register as a typedef plus one top-level instance.
    #[doc(hidden)]
    fn mc_register(&self, target: McRegisterTarget, instance_name: Option<&'static str>)
    where
        Self: Sized,
    {
        let ctx = McRegisterContext {
            target,
            instance_name,
            name_prefix: String::new(),
            addr_offset: 0,
            level: 0,
        };
        Self::register(&ctx);
    }

    /// Value-callable accessor for the type name (used by macros that only have a value).
    #[doc(hidden)]
    fn mc_type_name_value(&self) -> &'static str
    where
        Self: Sized,
    {
        Self::mc_type_name()
    }
}

//----------------------------------------------------------------------------------------------
// McEnumType

/// Implemented by `#[derive(McRegisterEnum)]` on an integer enum definition.
///
/// Carries the enum's backing integer value type (derived from its `#[repr(..)]`) and the A2L
/// verbal-conversion unit string built from the variant names and discriminants
/// (`0 "Off" 1 "On" 2 "Standby"`). A `#[characteristic(enum_type)]` field looks these up instead
/// of restating them at every use site.
/// Internal trait: used only by the generated `McRegisterType::register` code.
#[doc(hidden)]
pub trait McEnumType {
    /// The `McValueType` of the enum's backing integer (from `#[repr(..)]`).
    fn mc_value_type() -> McValueType;

    /// The A2L enum-format unit string: `<int> "<label>"` pairs joined by spaces.
    fn mc_enum_unit() -> &'static str;
}
