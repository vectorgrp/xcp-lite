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


**Resolved:** the flatten-vs-typedef choice is **not** decidable at runtime (there is no use
case for selecting it per call, and emitting both paths in every generated `register()` only
**Resolved:** the generated code always builds **typedefs** — the rich, complete
representation — and there is no flatten code path in the macro at all. Typedefs are the
progressive default; flattening is treated as a legacy workaround for tools that cannot consume
`TYPEDEF_STRUCTURE`. That workaround is implemented as a separate **export-time transform** on
the populated registry (an option of the A2L writer), **not** as a codegen mode. See §7 (model
and rationale) and §9 (decision). Both the `#[flatten]` attribute and a compile-time Cargo
feature are therefore unnecessary: the macro stays simple, the in-memory model is the single
source of truth, and the same build can emit either a typedef A2L or a flattened A2L.





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
- Always generate **typedefs** (the complete representation). Flattening for legacy tools that
  do not support typedefs is a separate, export-time transform on the registry — not a codegen
  mode (the transform itself is deferred; see §7).

Non-goals (for the first implementation, deferred to a later step):
- **Integer enum types.** These require a new *verbal* conversion rule (A2L `COMPU_VTAB` /
  `TAB_VERB`) that does not yet exist in `McSupportData` / the A2L writer. The macro will be
  designed so enums can be added later without breaking the public API (see §8).

## 2. Generated trait and method

The macro emits an implementation of a single trait. The trait method receives a small
context struct so the *same* generated code can register a calibration-segment-relative
typedef or an event-relative (measurement) typedef. The generated code always builds typedefs;
there is no flatten code path and no runtime mode argument. Flattening for legacy tools is a
separate, export-time transform on the registry (see §7).

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
    ///   accumulated name prefix and address offset (used during recursion into
    ///   nested typedefs), and the nesting level. There is **no** mode flag: the
    ///   generated code always builds typedefs (see §7/§9).
    fn register(ctx: &McRegisterContext);
}

/// Where and how to register. Internal type, not part of the public API.
#[doc(hidden)]
pub struct McRegisterContext {
    pub target: McRegisterTarget,   // CalSeg(name) | Event(id)
    pub instance_name: Option<&'static str>, // top-level instance name, None for nested
    pub name_prefix: String,        // accumulated "a.b." prefix when nesting
    pub addr_offset: u16,           // accumulated offset when nesting
    pub level: usize,               // recursion depth, 0 at top level
    // No `flatten` field: the macro always builds typedefs; flattening is an
    // export-time transform on the registry, not a codegen mode.
}
```

The blanket impls in `xcp_registry` (`register_calseg_typedef`, `register_calseg_fields`,
`register_struct_typedef`, `register_struct_fields`) are replaced by thin wrappers that build
an `McRegisterContext` and call `T::register(ctx)`. Backward compatibility with the old
`CalSeg` / registration entry-point signatures is **not** required, so these wrappers and the
`CalSeg` helper names may be redesigned freely for clarity.

### 2.1 User-facing wrappers

Users interact only with these wrappers; the context is hidden behind them. The macro always
registers typedefs (see §7/§9), so there is exactly one registration entry point per target —
no mode selection and no `*_typedef` / `*_deep` / `*_fields` variants:

- Calibration segment (in `CalSeg`):
  - `register(&self)` — builds a context with `target = CalSeg(name)` and registers the page
    as a typedef (nested structs become nested typedefs, arrays of structs become dimensioned
    typedef instances) plus one top-level instance named after the segment.
- Measurement struct (DAQ): the existing `daq_register_struct!` macro registers the stack
  instance with `target = Event(id)`, building the same typedef.

Each wrapper constructs the `McRegisterContext`, then calls the generated
`McRegisterType::register(&ctx)`. No other entry points are exposed. Flattened output for legacy
tools is produced later by the A2L writer transform, not by these wrappers.

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

## 7. Generation model: typedefs only; flattening as an export-time transform

The macro has a **single** code path: it always builds typedefs. This is the progressive,
complete representation and keeps the generated code minimal.

### Typedef generation (the only codegen behavior)

For each `#[derive(McRegisterType)]` struct the generated `register()`:

- ensures the typedef of every nested user struct exists first (recurse with `child_typedef()`,
  deeper `level`),
- creates this struct's typedef (`add_typedef`) and adds every field (`add_typedef_field`); a
  field whose type is a user struct uses the `TypeDef` value type, and an **array of structs**
  (`[S; N]` / `[[S; X]; Y]`) is the same `TypeDef` value type plus the array dimensions
  (`x_dim` / `y_dim`),
- at `level == 0`, if `instance_name` is `Some`, registers one top-level instance referencing
  the typedef.

The address mode (calseg-relative or event-relative) follows `ctx.target`.

### Flattening: a separate export-time transform (deferred)

Legacy tools that do not support `TYPEDEF_STRUCTURE` are served by a **lossy transform applied
to the populated registry at A2L-generation time**, not by the macro. The transform walks each
instance whose value type is a `TypeDef`, recursively expands its fields into dot-mangled leaf
instances (`calseg.test_struct.test_u8`; arrays of structs become `field._i.leaf`, where the
`._i.` dotted form is used because the A2L writer sanitizes `[`/`]` to `_`), recomputes
offsets, and drops the typedef definitions. The information it needs (typedef definitions and
field offsets) already lives in the registry, so the transform is a pure data pass.

Benefits of putting flattening here instead of in the macro or a Cargo feature:

- the derive macro and the in-memory model only ever speak the modern language (typedefs);
- the choice moves to **export time** (a writer option / builder setting), so no rebuild is
  needed to switch, and a single build can emit **both** a typedef A2L and a flattened A2L;
- the flatten logic lives once, operating on data, and is uniform across the whole type graph
  by construction.

> Status: the transform is **not yet implemented** in the A2L writer. Until it lands, only the
> typedef representation is produced. This section specifies the intended behavior.

### Removed: mixed mode

The earlier "flatten but keep a typedef instance for arrays of structs" behavior is dropped. A
tool that cannot consume typedefs cannot consume a typedef instance for a struct array either,
so when the flatten transform runs it deep-flattens struct arrays element-by-element.

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
  `#[doc(hidden)]`; users call the ergonomic `register()` wrapper on `CalSeg` and the
  `daq_register_struct!` macro, which build the context internally.
- **Generation always builds typedefs.** The macro has a single code path; there is no
  `flatten` flag in the context, no Cargo feature, and no `#[flatten]` attribute. `CalSeg`
  exposes a single `register()` wrapper (no `register_typedef` / `register_fields` /
  `register_fields_deep`). Flattening for legacy tools is a separate, export-time transform on
  the registry performed by the A2L writer (deferred; see §7), which deep-flattens struct
  arrays. Mixed mode is removed.

## 10. Open questions

### 10.1 A2L representation for arrays of a user-defined struct

A field whose type is a user struct becomes `McValueType::TypeDef("S")`. An **array** of such a
struct (e.g. `[UserDefinedType; 8]` or `[[UserDefinedType; 2]; 3]`) is represented by the same
`TypeDef` value type plus the array dimensions in `McDimType` (`x_dim` / `y_dim`). This is the
only representation the macro produces; the deferred export-time flatten transform (§7) would
later expand such arrays element-by-element (`field._i.leaf`) for legacy tools.

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


