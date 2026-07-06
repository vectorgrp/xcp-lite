# xcp_registry

Pure-Rust measurement and calibration (MC) registry for XCP.  
No C dependency — the crate holds the complete data model and drives A2L file generation.

The registry is the source of truth for all objects that are later described in an A2L file
and accessed by a calibration tool (CANape, etc.) over XCP.

---

## Contents

- [Overview](#overview)
- [Creating a Registry](#creating-a-registry)
- [Application metadata](#application-metadata)
- [Type definitions – `McTypeDef`](#type-definitions--mctypedef)
- [Instances – `McInstance`](#instances--mcinstance)
- [Support data – `McSupportData`](#support-data--mcsupportdata)
- [Addressing – `McAddress`](#addressing--mcaddress)
- [Writing A2L](#writing-a2l)
- [Reading A2L (optional feature)](#reading-a2l-optional-feature)
- [JSON serialization](#json-serialization)
- [Registry modes](#registry-modes)
- [Error handling](#error-handling)
- [Derive macro (`McRegisterType`)](#derive-macro-mcregistertype)

---

## Overview

The main types exported by this crate are:

| Type | Role |
|---|---|
| `Registry` | Root container – holds all MC objects |
| `McApplication` | Application name, description, EPK version string |
| `McTypeDef` / `McTypeDefField` | Struct-like type definition (maps to A2L `TYPEDEF_STRUCTURE`) |
| `McInstance` | Concrete measurement or calibration instance (maps to A2L `INSTANCE`, `MEASUREMENT`, `CHARACTERISTIC`, `AXIS_PTS`) |
| `McSupportData` | Rich metadata attached to a field or instance (unit, factor/offset, min/max, …) |
| `McValueType` | Scalar type enum or `McTypeDef` reference (`Ubyte`, `Sword`, `Float64Ieee`, `TypeDef(name)`, …) |
| `McDimType` | Type + optional array dimensions `[x][y]` |
| `McAddress` | XCP addressing information (cal-seg relative, event-relative, absolute) |
| `McCalibrationSegment` | A named contiguous memory region for calibration data |
| `McEvent` | DAQ event |

---

## Creating a Registry

```rust
use xcp_registry::Registry;

let mut registry = Registry::new();
```

Set optional rendering modes before populating (see [Registry modes](#registry-modes)):

```rust
registry.set_flatten_typedefs_mode(false); // default: false → use TYPEDEF_STRUCTURE
registry.set_prefix_names_mode(false);     // default: false → no app-name prefix
```

Set the XCP Ethernet transport parameters so the A2L gets an `IF_DATA XCP` section:

```rust
use std::net::Ipv4Addr;
registry.set_xcp_eth_params("UDP", Ipv4Addr::new(127, 0, 0, 1), 5555);
```

---

## Application metadata

`Registry::application` is an `McApplication` value.  
Set name, description, application id and version (EPK and EPK address) before writing the A2L:

```rust
registry.application.set_info("my_app", "My application description", 1);
registry.application.set_version("v1.2.3", 0x0000_0000);
```

---

## Type definitions – `McTypeDef`

A `McTypeDef` describes a struct type.  
Add one typedef per struct type, then add each struct field individually.

### Adding a typedef

```rust
use xcp_registry::{McSupportData, McObjectType, McValueType, McDimType};

// Register type "ControlParams" with its total size in bytes
let typedef = registry.add_typedef("ControlParams", std::mem::size_of::<ControlParams>())?;
```

`add_typedef` returns `Err(RegistryError::Duplicate)` when the same name already exists
(idempotent: the existing definition is kept).

### Adding fields

Each field needs:
- a field name (string)
- a `McDimType` describing its element type and optional array dimensions
- a `McSupportData` with metadata (at minimum: `McObjectType`)
- the byte offset of the field within the struct

```rust
use xcp_registry::{McSupportData, McObjectType, McValueType, McDimType};

// Scalar f64 field "gain" at byte offset 0, calibration characteristic
registry.add_typedef_field(
    "ControlParams",                     // typedef name
    "gain",                              // field name
    McDimType::new(McValueType::Float64Ieee, 1, 1),
    McSupportData::new(McObjectType::Characteristic)
        .set_unit("dB")
        .set_linear(1.0, 0.0, "dB")
        .set_min(Some(0.0))
        .set_max(Some(100.0)),
    0,                                   // offset in bytes
)?;

// u16 array [8] field "lut" at byte offset 8
registry.add_typedef_field(
    "ControlParams",
    "lut",
    McDimType::new(McValueType::Uword, 8, 1),
    McSupportData::new(McObjectType::Characteristic).set_unit("rpm"),
    8,
)?;

// Nested typedef: field "inner" of type "InnerStruct" at byte offset 24
registry.add_typedef_field(
    "ControlParams",
    "inner",
    McDimType::new(McValueType::new_typedef("InnerStruct"), 1, 1),
    McSupportData::new(McObjectType::Characteristic),
    24,
)?;
```

`add_typedef_field` returns `Err(RegistryError::Duplicate)` when a field with the same name
already exists (idempotent if the layout is identical; emits a warning on ABI mismatch).

### Full example – building a typedef manually

```rust
use xcp_registry::{Registry, McDimType, McValueType, McSupportData, McObjectType};

let mut registry = Registry::new();

// 1. Declare the type
registry.add_typedef("PidParams", 24)?;

// 2. Add fields
registry.add_typedef_field(
    "PidParams", "kp",
    McDimType::new(McValueType::Float64Ieee, 1, 1),
    McSupportData::new(McObjectType::Characteristic).set_unit("1"),
    0,
)?;
registry.add_typedef_field(
    "PidParams", "ki",
    McDimType::new(McValueType::Float64Ieee, 1, 1),
    McSupportData::new(McObjectType::Characteristic).set_unit("1/s"),
    8,
)?;
registry.add_typedef_field(
    "PidParams", "kd",
    McDimType::new(McValueType::Float64Ieee, 1, 1),
    McSupportData::new(McObjectType::Characteristic).set_unit("s"),
    16,
)?;
```

---

## Instances – `McInstance`

An `McInstance` binds a type (possibly a typedef) to a name and an address.  
Instances are added directly to the registry's `instance_list`.

```rust
use xcp_registry::{McDimType, McValueType, McSupportData, McObjectType, McAddress};

// Calibration instance of typedef "PidParams" relative to calibration segment "calseg"
registry.instance_list.add_instance(
    "pid",
    McDimType::new(McValueType::new_typedef("PidParams"), 1, 1),
    McSupportData::new(McObjectType::Characteristic),
    McAddress::new_calseg_rel("calseg", 0),  // byte offset within the calibration segment
)?;

// Scalar measurement instance of type f32, event-relative (dynamic DAQ)
registry.instance_list.add_instance(
    "speed",
    McDimType::new(McValueType::Float32Ieee, 1, 1),
    McSupportData::new(McObjectType::Measurement)
        .set_unit("km/h")
        .set_min(Some(0.0))
        .set_max(Some(400.0)),
    McAddress::new_event_dyn(0, event_id, offset_of_speed),
)?;
```

### Addressing constructors

| Constructor | Use case |
|---|---|
| `McAddress::new_calseg_rel(seg_name, offset)` | Calibration segment–relative (characteristic / axis) |
| `McAddress::new_event_dyn(index, event_id, offset)` | Event-relative dynamic DAQ (measurement) |
| `McAddress::new_event_abs(event_id, offset)` | Event-relative absolute addressing |
| `McAddress::new_a2l(addr, addr_ext)` | Raw XCP address from a third-party A2L |
| `McAddress::new_a2l_with_event(event_id, addr, addr_ext)` | Raw XCP address with event association |

### Updating metadata on a registered instance

Use `get_instance_mut` to obtain a mutable reference, then call the `update_*` methods
on `mc_support_data` (which take `&mut self` and return `&mut Self` for chaining):

```rust
if let Some(inst) = registry.instance_list.get_instance_mut("speed", None) {
    inst.mc_support_data
        .update_unit("m/s")
        .update_linear(1.0 / 3.6, 0.0, "m/s")  // physical = raw / 3.6
        .update_min(Some(0.0))
        .update_max(Some(111.0))
        .update_comment("Vehicle speed in m/s");
}
```

The second argument to `get_instance_mut` is an optional `event_id` filter
(`None` matches any event).

### Updating metadata on a typedef field (simple — when you know the typedef name)

Use `find_typedef_mut` on the typedef list, then `find_field_mut` on the typedef:

```rust
if let Some(field) = registry.typedef_list
    .find_typedef_mut("PidParams")
    .and_then(|td| td.find_field_mut("kp"))
{
    field.mc_support_data
        .update_unit("1")
        .update_linear(0.001, 0.0, "1")
        .update_min(Some(0.0))
        .update_max(Some(100.0))
        .update_comment("Proportional gain");
}
```

### Updating metadata via instance + field path (preferred for complex/nested types)

When an instance has a complex type and the target field may be arbitrarily deep inside
nested typedefs, use `Registry::set_instance_field_support_data`.  The field is addressed
by a **dot-separated path** relative to the instance's typedef — field names live in their
typedef's namespace, not a flat global namespace.

```rust
// Instance "pid" has typedef "PidController"
//   PidController::gains  →  typedef "GainStruct"
//     GainStruct::kp      →  f64

registry.set_instance_field_support_data(
    "pid",           // instance name
    "gains.kp",      // dot-separated path through nested typedefs
    McSupportData::new(McObjectType::Characteristic)
        .set_unit("1")
        .set_linear(0.001, 0.0, "1")
        .set_min(Some(0.0))
        .set_max(Some(100.0)),
)?;

// Direct field (no nesting):
registry.set_instance_field_support_data(
    "speed_sensor",
    "raw_value",
    McSupportData::new(McObjectType::Measurement)
        .set_unit("km/h"),
)?;
```

**Key behaviours:**
- If the caller sets `object_type` to `Unspecified`, the existing `object_type` of the field
  is preserved automatically (safe to pass `McSupportData::default()` with only specific
  fields set via `update_*`).
- Returns `Err(RegistryError::MetadataAlreadySet)` if the field already has descriptive
  metadata (unit / min / max / factor / offset / step / comment non-empty).  This enforces
  the current data-model limitation: typedef fields are **shared** across all instances of
  the same type, so conflicting assignments are rejected rather than silently overwriting.
- Returns `Err(RegistryError::NotFound)` if the instance, its typedef, or any path
  component does not exist.

**Limitation:** because typedef fields are shared, setting metadata here affects every
instance that uses the same typedef — there is no per-instance override yet.

---

## Support data – `McSupportData`

`McSupportData` carries the rich metadata that ends up in the A2L object description.

Two distinct setter families are available:

| Family | Signature | When to use |
|---|---|---|
| Builder (`set_*`) | `self -> Self` | At construction time — chain on `McSupportData::new(...)` |
| Mutation (`update_*`) | `&mut self -> &mut Self` | Post-construction — update an already-registered instance |

### Required field

```rust
McSupportData::new(McObjectType::Measurement)   // Measurement | Characteristic | Axis
```

### Builder chain (construction time)

```rust
McSupportData::new(McObjectType::Characteristic)
    .set_unit("°C")                     // physical unit string
    .set_linear(0.1, -40.0, "°C")      // factor=0.1, offset=-40.0, implicit set_unit
    .set_min(Some(-40.0))               // lower display limit
    .set_max(Some(150.0))               // upper display limit
    .set_step(Some(0.1))                // calibration step width
    .set_comment("Coolant temperature")  // free-text description
    .set_qualifier(McObjectQualifier::Volatile) // mark as volatile
```

### Mutation chain (post-registration)

```rust
// Update metadata on an instance that was already added to the registry
if let Some(inst) = registry.instance_list.get_instance_mut("temp", None) {
    inst.mc_support_data
        .update_linear(0.1, -40.0, "°C")
        .update_min(Some(-40.0))
        .update_max(Some(150.0))
        .update_comment("Coolant temperature");
}
```

`update_*` methods available: `update_unit`, `update_linear`, `update_min`, `update_max`,
`update_step`, `update_comment`.

### Object qualifier

`McObjectQualifier` refines the volatility and access types of an object independently of its type:

| Variant | Meaning |
|---|---|
| `Unspecified` | Default – calibration objects are assumed constant |
| `Volatile` | Continuously modified by the target (typical for measurements) |
| `ReadOnly` | No asynchronous write possible |
| `NoAsyncAccess` | Assumed volatile, no direct write |

### Axis references

For maps/curves whose axes are separate objects:

```rust
McSupportData::new(McObjectType::Characteristic)
    .set_x_axis_ref(Some("speed_axis".into()))
    .set_y_axis_ref(Some("temp_axis".into()))
```

---

## Writing A2L

```rust
registry.write_a2l(
    "my_app.a2l",           // output file path
    "/* generated */",      // title comment
    "MY_PROJECT",           // A2L PROJECT name
    "My application",       // PROJECT description
    "MY_MODULE",            // A2L MODULE name
    "1.0",                  // project number
    false,                  // check: re-read and validate after writing (requires feature a2l_reader)
)?;
```

The writer emits `TYPEDEF_STRUCTURE` / `INSTANCE` blocks by default.
With `set_flatten_typedefs_mode(true)` the registry is internally flattened before
writing, producing plain `MEASUREMENT` / `CHARACTERISTIC` / `AXIS_PTS` objects with mangled names instead.

---

## Reading A2L (optional feature)

Enable the `a2l_reader` Cargo feature to load or validate A2L files:

```toml
[dependencies]
xcp_registry = { version = "3", features = ["a2l_reader"] }
```

### Load an A2L file into a registry

```rust
let mut registry = Registry::new();
let warnings = registry.load_a2l(
    "my_app.a2l",
    true,    // print_warnings
    false,   // strict parsing
    true,    // run consistency check
    false,   // flatten typedefs after loading
)?;
println!("{} warnings", warnings);
```

### Validate an A2L file (syntax + consistency check)

```rust
let warnings = registry.check_a2l("my_app.a2l")?;
```

---

## JSON serialization

The registry can be round-tripped through JSON (useful for testing or offline tooling):

```rust
// Serialize to JSON
registry.write_json(&"registry.json")?;

// Deserialize from JSON
let mut registry = Registry::new();
registry.load_json(&"registry.json")?;
```

`McSupportData` also has dedicated string helpers for embedding in other JSON structures:

```rust
let s: String = mc_support_data.to_json_string();
let m: Option<McSupportData> = McSupportData::from_json_string(&s);
```

---

## Registry modes

| Method | Default | Effect |
|---|---|---|
| `set_flatten_typedefs_mode(bool)` | `false` | When `true`, nested typedefs are flattened to flat objects with mangled names at A2L export time |
| `set_prefix_names_mode(bool)` | `false` | When `true`, every A2L object name is prefixed with the application name |

Flattening only affects the *export*; the internal registry structure (typedef tree) is
preserved regardless.

---

## Error handling

All mutating registry calls return `Result<_, RegistryError>`:

```rust
use xcp_registry::RegistryError;

match registry.add_typedef("MyStruct", 16) {
    Ok(_) => { /* newly created */ }
    Err(RegistryError::Duplicate(_)) => { /* already exists – fine to ignore */ }
    Err(e) => return Err(e.into()),
}
```

| Variant | Meaning |
|---|---|
| `RegistryError::Duplicate(name)` | Object with that name already registered |
| `RegistryError::NotFound(name)` | Referenced object (e.g. typedef for a field) does not exist |
| `RegistryError::UnknownEventChannel(id)` | Event id not defined |
| `RegistryError::Io(e)` | File I/O error |

---

## Derive macro (`McRegisterType`)

In practice you rarely call the low-level API above directly.  
The `#[derive(McRegisterType)]` proc-macro (crate `xcp_register_type_derive`) generates
the necessary `add_typedef` / `add_typedef_field` / `add_instance` calls for a Rust struct.

### Minimal example

```rust
use xcp_registry::McRegisterType;

#[derive(McRegisterType)]
struct PidParams {
    kp: f64,
    ki: f64,
    kd: f64,
}
```

This generates code equivalent to the manual example in the [Type definitions](#type-definitions--mctypedef) section.

### Field attributes

Override the default classification or add metadata per field:

```rust
#[derive(McRegisterType)]
struct ControlParams {
    #[characteristic(unit = "dB", min = 0.0, max = 100.0, comment = "Proportional gain")]
    gain: f64,

    #[measurement(unit = "rpm", comment = "Observed speed")]
    speed: f32,

    #[axis(unit = "rpm")]
    speed_axis: [f32; 16],
}
```

Supported attribute names: `#[characteristic(...)]`, `#[measurement(...)]`, `#[axis(...)]`.  
Supported keys inside an attribute: `unit`, `comment`, `min`, `max`, `step`, `factor`, `offset`.

### Using the derive with `CalSeg` (xcp-lite)

When working through the higher-level `xcp-lite` crate the derive is invoked automatically
via `CalSeg::register()` and the `daq_register_struct!` macro; you do not call the registry
methods directly.
