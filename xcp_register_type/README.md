# XCP Register Type proc_macro


New proc macro.

Goal:

Create a new optimized proc_macro called McRegisterType.
The new macro should generate code to register the types directly in the registry.
The old macro in xcp_type_description generated intermediate data structures to register the types later in the mc_registry. The type was 

```rust
struct StructDescriptor {
    name: &'static str,
    size: usize,
    fields: Vec<FieldDescriptor>,
}
```

The new macro should generate code that directly registers the types in the registry.
It could support more types than the old version. The new macro should at least support the following types:
- bool
- u8, u16, u32, u64
- i8, i16, i32, i64
- f32, f64
- arrays of the above types (up to 2 dimensions)
- user defined types (structs) that struct are also registered in the registry recursivly or flattened as an option
- arrays of the user defined types (up to 2 dimensions)
- Integer enum types - these are registered as u8, u16, u32, u64 depending on the size of the enum and a conversion rule which describes the value names. Postponed to a later step, the macro should be designed so that enums can be added later without breaking the public API.


The attribute parser should be more robust than before.
The new macro is intentionally **not** syntax-compatible with the old macro; a cleaner, more user-friendly syntax is preferred over backward compatibility. The new macro should support the following attributes:

- `#[characteristic(comment = "Demo comment")]` - optional comment for the characteristic
- `#[characteristic(min = 0, max = 100)]` - optional min and max values for the characteristic
- `#[characteristic(unit = "s")]` - optional physical unit for the characteristic
- `#[characteristic(axis = "path.to.axis")]` - optional axis for the characteristic, used for curves
- `#[characteristic(x_axis = "path.to.x_axis")]` - optional x axis for the characteristic, used for maps
- `#[characteristic(y_axis = "path.to.y_axis")]` - optional y axis for the characteristic, used for maps
- `#[characteristic(step = 10)]` - optional step size for the characteristic, used for curves and maps with fixed axis min..max

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

Both the trait method and the context struct are **internal** (`#[doc(hidden)]`). End users
never construct a context or call `register` directly; they call ergonomic wrappers that build
the context internally (see §2.1).

```rust
/// Implemented by #[derive(McRegisterType)]. Internal: called only by the wrappers.
#[doc(hidden)]
pub trait McRegisterType {
    /// Register this type into the open registry singleton.
    ///
    /// * `ctx` carries the registration target (calseg name or event id), the
    ///   accumulated name prefix and address offset (used during recursion and
    ///   flattening), the nesting level, and the `flatten` flag.
    fn register(ctx: &McRegisterContext);
}

/// Where and how to register. Internal type, not part of the public API.
#[doc(hidden)]
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

### 2.1 User-facing wrappers

Users interact only with these wrappers; the context is hidden behind them. The wrappers keep
the familiar names so the call sites read the same as today:

- Calibration segment (in `CalSeg`):
  - `register_typedef(&self)` — build a context with `target = CalSeg(name)`, `flatten = false`,
    `instance_name = Some(type/seg name)`; registers one typedef plus one instance.
  - `register_fields(&self)` — same target but `flatten = true`; registers flattened,
    dot-mangled instances and no typedef.
- Measurement struct (DAQ): the existing `daq_register_struct!` macro calls a typedef wrapper
  with `target = Event(id)` internally and then registers the stack instance.

Each wrapper constructs the `McRegisterContext`, then calls the generated
`McRegisterType::register(&ctx)`. No other entry points are exposed.

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

The syntax is a single attribute per field with all keys combined and native literals.
Numeric keys take **numeric literals** (not strings), and text keys take string literals:

```rust
#[characteristic(comment = "Demo float", min = -10.0, max = 10.0, unit = "s")]
float: f32,
```

This is a deliberate break from the old macro, where every value (including numbers) had to
be a quoted string. Each field carries **at most one** classifier attribute; repeating a
classifier attribute on the same field is a `compile_error!` (this keeps the parser simple).

Recognized attribute paths: `characteristic`, `axis`, `measurement`. Any other attribute on a
field is ignored (it may belong to another derive macro). Keys map to `McSupportData` setters:

| Key | Value kind | Applies to | McSupportData target |
| --- | --- | --- | --- |
| `comment` | string | all | `set_comment` |
| `min`, `max` | number | all | `set_min`, `set_max` |
| `step` | number | characteristic | `set_step` |
| `unit` | string | all | `set_linear(factor, offset, unit)` |
| `factor`, `offset` | number | all | `set_linear` |
| `qualifier` | string: `"volatile"` \| `"readonly"` | all | `set_qualifier(Volatile \| ReadOnly)` |
| `axis` | string | characteristic (curve) | `set_x_axis_ref` |
| `x_axis`, `y_axis` | string | characteristic (map) | `set_x_axis_ref`, `set_y_axis_ref` |
| `input_quantity` (alias `x_input_quantity`) | string | characteristic (curve/map) | `set_x_axis_input_quantity` |
| `y_input_quantity` | string | characteristic (map) | `set_y_axis_input_quantity` |

Notes:
- **`qualifier`** maps to `McObjectQualifier`: `"volatile"` → `Volatile` (continuously modified
  by the target), `"readonly"` → `ReadOnly` (no async write, assumed volatile). Unspecified
  otherwise. The old macro also recognized `readonly`; it is kept here.
- **Input quantity** (`input_quantity` / `y_input_quantity`) names the measurement signal that
  addresses the axis of a curve or map. These correspond to the old macro's `x_axis_inputQty`
  / `axis_inputQty` / `y_axis_inputQty` keys, renamed to clean snake_case (the new macro is not
  backward-compatible, see §1). On an `#[axis(...)]` field they are ignored, exactly like the
  axis-ref keys.

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
- **Context is internal.** `McRegisterContext` and `McRegisterType::register` are
  `#[doc(hidden)]`; users call ergonomic wrappers (`register_typedef` / `register_fields` on
  `CalSeg`, and the `daq_register_struct!` macro) that build the context internally.

## 10. Open questions

### 10.1 A2L representation for arrays of a user-defined struct

A field whose type is a user struct becomes `McValueType::TypeDef("S")`. An **array** of such a
struct (e.g. `[UserDefinedType; 8]` or `[[UserDefinedType; 2]; 3]`) is represented by the same
`TypeDef` value type plus the array dimensions in `McDimType` (`x_dim` / `y_dim`).

The A2L writer already emits these as a single `INSTANCE ... <TypeName> ... MATRIX_DIM x [y]`
(see `McInstance::write_measurement` and `write_matrix_dim` in
`xcp_registry/src/a2l/a2l_writer.rs`). The open item is only to **confirm** the desired
`x_dim`/`y_dim` convention for struct arrays matches what the calibration tool (CANape) expects
— i.e. that `[S; X]` → `x_dim = X, y_dim = 1` and `[[S; X]; Y]` → `x_dim = X, y_dim = Y`, the
same convention as for scalar arrays. No new registry code is expected; this is a verification
against tooling.

## 11. Packaging / crate layout

### 11.1 Why a separate proc-macro crate

A crate with `proc-macro = true` (like `xcp_type_description_derive` and
`xcp_idl_generator_derive`) can export **only** proc-macros — no traits, structs, or
functions. So the derive itself must live in its own crate. This follows the existing
`<lib>` + `<lib>_derive` convention in the workspace.

### 11.2 Where each piece lives

The new design removes all intermediate runtime types (no `StructDescriptor` /
`FieldDescriptor`), so the only runtime artifact is the (doc-hidden) trait plus its context.
Its natural home is **`xcp_registry`**, which already hosts the analogous `RegisterFieldsTrait`
and all the `Mc*` support types. This means **no new runtime library crate** is needed — only
the mandatory proc-macro crate.

```
xcp_registry/              # add McRegisterType trait + McRegisterContext (#[doc(hidden)]);
                           # depend on the derive crate and re-export it (serde-style)
xcp_register_type_derive/  # proc-macro = true; ONLY the derive
                           # deps: syn, quote, proc-macro2 (NOT xcp_registry)
```

Key point: the **proc-macro crate does not depend on `xcp_registry`**. The generated code only
emits absolute paths like `::xcp_registry::McValueType` / `::xcp_registry::add_typedef(...)` as
tokens; the dependency is satisfied in the **consuming** crate, which already pulls in
`xcp_registry`. There is therefore no dependency cycle, even though `xcp_registry` re-exports
the derive.

### 11.3 User import

By re-exporting the derive from `xcp_registry` (the serde model: `serde::Serialize` is both a
trait and, via re-export, the derive), users get both from one path:

```rust
use xcp_registry::McRegisterType;   // brings the trait AND the derive macro
```

Alternative (more decoupled): do not re-export from `xcp_registry`; instead aggregate in the
root `xcp_lite` prelude (`src/lib.rs` already does `pub use xcp_type_description::prelude::*`).
Slightly less ergonomic for standalone `xcp_registry` users.

### 11.4 Naming

- Keep **snake_case** crate names (`xcp_registry`, `xcp_register_type_derive`) — consistent
  with the rest of the workspace. Note crates.io treats `-` and `_` as colliding for
  uniqueness; do not mix styles.
- The `Mc*` type prefix with `xcp_*` crate prefix is the established pattern (`xcp_registry`
  exports `McValueType`, `McSupportData`, …), so `McRegisterType` fits.

### 11.5 Publishing the monorepo on crates.io

- Each workspace member is published **independently** (`cargo publish` per crate) with its
  own version.
- **Path dependencies need a version** as well, e.g.
  `xcp_register_type_derive = { path = "...", version = "1.1.0" }`; crates.io ignores `path`
  and uses `version`.
- Publish **leaf-first**: `xcp_register_type_derive` → `xcp_registry` → `xcp_lite`.
- Mark non-published members with `publish = false` (the `examples/*`, `tools/xcp_client`).
- Every published crate needs `description` / `license` / `repository`. `xcp_registry` already
  has them; the existing `*_derive` `Cargo.toml`s do **not** and would need those fields added
  before publishing — apply the same to the new derive crate.


