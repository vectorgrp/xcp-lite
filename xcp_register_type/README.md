# XCP Register Type proc_macro


New proc macro.

Goal:

Create a new optimized proc_macro called McRegisterType.
The new macro should generate code to register the types directly in the mc_registry.
The old macro in xcp_type_description generated intermediate data structures to register the types later in the mc_registry. The type was 
```rust
struct StructDescriptor {
    name: &'static str,
    size: usize,
    fields: Vec<FieldDescriptor>,
}
```
The new macro should generate code that directly registers the types in the mc_registry if possible.
It should support also support more types than the old version. The new macro should support the following types:
- bool
- u8, u16, u32, u64
- i8, i16, i32, i64
- f32, f64
- arrays of the above types (up to 2 dimensions)
- user defined types (structs) that are also registered in the mc_registry
- arrays of the user defined types (up to 2 dimensions)
- Integer enum types - these are registered as u8, u16, u32, u64 depending on the size of the enum and a conversion rule which describes the value names.


The attribute parser should be more robust.
The new macro is intentionally **not** syntax-compatible with the old macro; a cleaner, more
user-friendly syntax is preferred over backward compatibility. The new macro should support
the following attributes:
- `#[characteristic(comment = "Demo comment")]` - optional comment for the characteristic
- `#[characteristic(min = 0, max = 100)]` - optional min and max values for the characteristic
- `#[characteristic(unit = "s")]` - optional physical unit for the characteristic
- `#[characteristic(axis = "path.to.axis")]` - optional axis for the characteristic, used for curves and maps
- `#[characteristic(x_axis = "path.to.x_axis")]` - optional x axis for the characteristic, used for maps
- `#[characteristic(y_axis = "path.to.y_axis")]` - optional y axis for the characteristic, used for maps
- `#[characteristic(step = 10)]` - optional step size for the characteristic, used for curves and maps

- `#[axis(comment = "Demo comment")]` - optional comment for the axis
- `#[axis(min = 0, max = 100)]` - optional min and max values for the axis
- `#[axis(unit = "s")]` - optional physical unit for the axis


Unclear how to deal with the option to flatten the type hierarchy. The old macro had an option to register the type with flat the type hierarchy, which means that the fields of the user defined types are registered as characteristics in the mc_registry and the offset was recalculated. The new macro should support this option as well, but it is unclear how to implement it. Maybe a new attribute `#[flatten]` can be used to indicate that the type should be flattened, but there must still be a way to decide when calling the generated code.





Example of a proc_macro that registers a type in the xcp_registry:

```rust


#[derive(Debug, Clone, Copy, McRegisterType)]
struct UserDefinedType {
    #[characteristic(comment = "Demo user defined type")]
    field1: u32,
    #[characteristic(comment = "Demo user defined type")]
    field2: u32,
}

#[derive(Debug, Clone, Copy, McRegisterType)]
struct Params {

    // Boolean field with comment
    #[characteristic(comment = "Demo boolean")]
    boolean: bool, 

    // Characteristics:
    // Characteristics are tunable parameters that can be read and written via XCP. They are defined using the `#[characteristic]` attribute, which can include optional metadata such as comments, minimum and maximum values, and axis information for curves and maps.

    // Unsigned 32-bit field with comment and min/max values
    #[characteristic(comment = "Demo integer")]
    #[characteristic(min = "0", max = "10000")]
    integer: u32, 
     
     // Float field with comment and min/max values and a physical unit
    #[characteristic(comment = "Demo float", min = "-10.0", max = "10.0")]
    #[characteristic(unit = "s")]
    float: f32,

    // Signed 32-bit array field with dimension 4 and comment and min/max values
    #[characteristic(comment = "Demo array", min = "0", max = "100")]
    array: [u8; 4], 

    // 2D array field with dimension 5x9 and comment and min/max values
    #[characteristic(comment = "Demo matrix", min = "0", max = "100")]
    matrix: [[u8; 9]; 5],

    // Axis and Curves/Maps:
    // Curves and maps are specialized multi-dimensional characteristics that are used as lookup tables that can be defined with shared axes. 
    // The `#[axis]` attribute is used to define a shared axes used as axis for a one or two-dimensional characteristic

    // Axis 
    #[axis(comment = "Demo shared axis", min = "0", max = "10000")]
    shared_axis_16: [f32; 16], 
    #[axis(comment = "Demo shared axis", min = "0", max = "10000")]
    shared_axis_9: [f32; 9],

    // Curves 
    #[characteristic(comment = "Demo curve with shared axis", axis = "cal_demo_2.params.shared_axis_16", min = "-10", max = "10")]
    curve1: [f64; 16], // CURVE type (1 dimension), shared axis 'shared_axis_16'
    #[characteristic(comment = "Demo curve with shared axis", axis = "cal_demo_2.params.shared_axis_16", min = "-10", max = "10")]
    curve2: [f64; 16], // CURVE type (1 dimension)

    // Maps
    #[characteristic(comment = "Demo map with shared axis", min = "0", max = "100")]
    #[characteristic(x_axis = "cal_demo_2.params.shared_axis_9")]
    #[characteristic(y_axis = "cal_demo_2.params.shared_axis_16")]
    map: [[u8; 9]; 16], // MAP type (2 dimensions), shared axis 'shared_axis_9' and 'shared_axis_16'

    // User defined types:
    // User defined types can be registered in the xcp_registry using the `#[xcp_type]` attribute. This allows for the creation of complex data structures that can be serialized and deserialized via XCP. The `#[xcp_type]` attribute can be used to specify the name of the type in the registry, as well as optional metadata such as comments and min/max values

    user_defined: UserDefinedType, // User defined type

    // Multi dimensional user defined types can also be registered in the xcp_registry 

    #[characteristic(comment = "Demo multi-dimensional user defined type")]
    user_defined_type_array: [[UserDefinedType; 2]; 3], // 2 dimensional array of user defined type
    user_defined_type_matrix: [UserDefinedType; 8], // 1 dimensional array of user defined type



}


````

---

# Design Specification

This section specifies the design of the new `McRegisterType` derive macro. It is the
authoritative reference for the implementation. Where it differs from the older
`XcpTypeDescription` macro, the difference is called out explicitly.

## 1. Goals and non-goals

Goals:
- Generate code that registers a struct type **directly** in the `xcp_registry` (`mc_registry`),
  with no intermediate `StructDescriptor` / `FieldDescriptor` data structures.
- Support all scalar types, arrays (up to 2 dimensions), nested user-defined struct types,
  and arrays of user-defined types.
- Provide a robust, user-friendly attribute parser. **The macro is intentionally not
  syntax-compatible with the old `XcpTypeDescription` macro**; backward compatibility is
  explicitly dropped in favor of a cleaner, less error-prone syntax (see §5).
- Keep the classifiers `characteristic`, `axis`, and `measurement`.
- Preserve the runtime choice between **typedef** registration and **flattened** registration.

Non-goals (for the first implementation, deferred to a later step):
- **Integer enum types.** These require a new *verbal* conversion rule (A2L `COMPU_VTAB` /
  `TAB_VERB`) that does not yet exist in `McSupportData` / the A2L writer. The macro will be
  designed so enums can be added later without breaking the public API (see §8).

## 2. Generated trait and method

The macro emits an implementation of a single trait. The trait method receives a small
context struct so the *same* generated code can register either a calibration-segment-relative
typedef, an event-relative (measurement) typedef, or a flattened set of instances. The
flatten decision is a **runtime argument** (matching the old `flat: bool`), not an attribute,
because the README requires that the decision be made at the call site.

```rust
/// Implemented by #[derive(McRegisterType)]
pub trait McRegisterType {
    /// Register this type into the open registry singleton.
    ///
    /// * `ctx` carries the registration target (calseg name or event id), the
    ///   accumulated name prefix and address offset (used during recursion and
    ///   flattening), the nesting level, and the `flatten` flag.
    fn register(ctx: &McRegisterContext);
}

/// Where and how to register. Construction helpers mirror the existing
/// CalSeg::register_typedef() / register_fields() entry points.
pub struct McRegisterContext {
    pub target: McRegisterTarget,   // CalSeg(name) | Event(id)
    pub instance_name: Option<&'static str>, // top-level instance name, None for nested
    pub name_prefix: String,        // accumulated "a.b." prefix when flattening
    pub addr_offset: u16,           // accumulated offset when flattening / nesting
    pub level: usize,               // recursion depth, 0 at top level
    pub flatten: bool,              // true => emit instances, false => emit typedef
}
```

The blanket impls in `xcp_registry` (`register_calseg_typedef`, `register_calseg_fields`,
`register_struct_typedef`, `register_struct_fields`) are replaced by thin wrappers that build
an `McRegisterContext` and call `T::register(ctx)`. Backward compatibility with the old
`CalSeg` / registration entry-point signatures is **not** required, so these wrappers and the
`CalSeg` helper names may be redesigned freely for clarity.

> Note: scalar primitives (`u8`..`f64`, `bool`) and arrays do **not** implement
> `McRegisterType`. The macro decides statically (from the field's syntactic type) whether a
> field is a primitive/array or a nested user type, so no blanket primitive impl and no
> trait-based dispatch at runtime is required. This is the key simplification over the old
> design, which used `<T as XcpTypeDescription>::type_description(...)` returning `Option`.

## 3. Direct registry API targets

The generated code calls only these existing registry functions (no new registry API needed
for the first implementation):

| Purpose | Call |
| --- | --- |
| Create a typedef | `reg.add_typedef(type_name, size_of::<T>())` |
| Add a field to a typedef | `reg.add_typedef_field(type_name, field_name, McDimType, McSupportData, addr_offset)` |
| Register an instance | `reg.instance_list.add_instance(name, McDimType, McSupportData, McAddress)` |

Addresses use `McAddress::new_calseg_rel(calseg, offset)` (calibration) or
`McAddress::new_event_dyn(0, event_id, offset)` (measurement). The lock is obtained via
`registry::get_lock()`.

## 4. Type mapping

The field's Rust type is mapped at compile time:

- Scalars: `bool, u8, u16, u32, u64, i8, i16, i32, i64, f32, f64` →
  `McValueType::{Bool, Ubyte, …, Float64Ieee}`.
- Arrays up to **2 dimensions** (the `mc_registry` `McDimType` carries only `x_dim` /
  `y_dim`): the element type determines `McValueType`; the array lengths determine
  `x_dim` / `y_dim`. `[T; X]` → `x_dim = X`, `y_dim = 1`; `[[T; X]; Y]` → `x_dim = X`,
  `y_dim = Y`. There is **no dimension folding** (the old macro's 3D→`y_dim` folding was
  unintended). A 3-or-more dimensional array is a `compile_error!`.
- User-defined struct `S` → `McValueType::new_typedef("S")`; the macro recurses into
  `<S as McRegisterType>::register(ctx_child)` first to ensure the typedef exists.
- Arrays of user-defined structs → `McValueType::TypeDef("S")` with the array dims as above.

The size of a typedef is `std::mem::size_of::<T>()`; field offsets are
`std::mem::offset_of!(T, field)` cast to `u16` (both `const`-evaluable).

## 5. Attribute syntax

The canonical syntax is a single combined attribute per classifier with native literals.
Numeric keys take **numeric literals** (not strings), and text keys take string literals:

```rust
#[characteristic(comment = "Demo float", min = -10.0, max = 10.0, unit = "s")]
float: f32,
```

This is a deliberate break from the old macro, where every value (including numbers) had to
be a quoted string. As a convenience, repeating the same classifier attribute is allowed and
the key sets are merged, but it is not the recommended style:

```rust
#[characteristic(comment = "Demo float")]
#[characteristic(min = -10.0, max = 10.0)]
float: f32,
```

Recognized attribute paths: `characteristic`, `axis`, `measurement`. Any other attribute on a
field is ignored (it may belong to another derive macro). Keys map to `McSupportData` setters:

| Key | Value kind | Applies to | McSupportData target |
| --- | --- | --- | --- |
| `comment` | string | all | `set_comment` |
| `min`, `max` | number | all | `set_min`, `set_max` |
| `step` | number | characteristic | `set_step` |
| `unit` | string | all | `set_linear(factor, offset, unit)` |
| `factor`, `offset` | number | all | `set_linear` |
| `qualifier = "volatile"` | string | all | `set_qualifier(Volatile)` |
| `axis` | string | characteristic (curve) | `set_x_axis_ref` |
| `x_axis`, `y_axis` | string | characteristic (map) | `set_x_axis_ref`, `set_y_axis_ref` |

Parser robustness requirements (improvements over the old parser, which `panic!`s freely):
- Emit `compile_error!` with a clear message and the offending span instead of `panic!`.
- Reject unknown keys, duplicate keys, and keys not valid for the given classifier.
- Require the correct literal kind per key: numeric keys reject string literals and vice
  versa, with a clear diagnostic.

## 6. Classifiers and object type

- `#[characteristic(...)]` → `McObjectType::Characteristic`
- `#[axis(...)]` → `McObjectType::Axis` (axis fields ignore `x_axis`/`y_axis` refs)
- `#[measurement(...)]` → `McObjectType::Measurement` (kept for compatibility)
- No attribute → default depends on target: `Characteristic` for calseg, `Measurement` for
  event (same defaulting rule as the current `register_as`).

## 7. Flatten algorithm (runtime `flatten == true`)

When `flatten` is true, no typedef is created. Instead every (possibly nested) leaf field is
registered as its own instance, mirroring the current `register_as` flatten path:

- The instance name is `name_prefix + field_name` (dot-separated), e.g.
  `calseg.test_struct.test_u8`.
- The address offset is `ctx.addr_offset + offset_of!(T, field)`.
- For a nested user-defined struct field, recurse with an extended prefix and accumulated
  offset; leaf scalars/arrays are emitted as instances.
- Address mode is calseg-relative or event-relative according to `ctx.target`.

When `flatten` is false, a typedef is created and one top-level instance referencing the
typedef is registered (only at `level == 0` and only if `instance_name` is `Some`).

## 8. Deferred: integer enums (planned, not in first implementation)

Planned approach so the API stays forward-compatible:
1. Add a verbal conversion rule to `McSupportData` (e.g. `set_enum(&[(i64, &'static str)])`)
   and emit `COMPU_VTAB` / `TAB_VERB` in `a2l_writer.rs`.
2. The macro detects an `enum` deriving `McRegisterType`, picks the backing
   `McValueType` from `size_of` (`u8/u16/u32/u64`), and registers the variant
   value→name pairs as the conversion rule.
No public-API change to `McRegisterType::register` is required to add this later.

## 9. Resolved decisions

- **Numeric literals** are the canonical form for numeric keys (`min`, `max`, `step`,
  `factor`, `offset`); strings are only for text keys.
- **No backward compatibility** with the old macro's syntax or with the old registration
  entry-point signatures; the API may be redesigned for clarity.
- **Maximum 2 array dimensions**; 3+ dimensions are a `compile_error!` and there is no
  dimension folding.

## 10. Open questions

- Final naming of the public entry points (the redesigned `CalSeg` / context helpers).
- Confirm the desired A2L representation for a 1D array of a user-defined struct vs. a 2D
  array of a struct (instance dims on a `TypeDef` value type).

