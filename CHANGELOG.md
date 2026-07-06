# Changelog

All notable changes to Rust xcp-lite are documented in this file.


## [V3.0.1]

  - Support for enum fields of basic types (u8, u16, u32, i8, i16, i32) in the new McRegisterType system.
    - The `enum_type` attribute on an enum field specifies the underlying integer type for the enum.
    - The `unit` attribute on an enum field specifies the label mapping for the enum values (e.g. `0 "OFF" 1 "ON" 2 "STANDBY"`).
    - The `McRegisterType` derive macro generates the necessary code to register the enum field with its underlying type and labels in the registry.
    - The `xcp_registry` crate has been updated to support enum fields in the registry and A2L generation.
    - The `hello_xcp` example has been updated to demonstrate enum fields with basic types.
  - Deterministic calibration segment registration via the new `linkme` feature (enabled by default).
    The new `cal_seg!("name", &DEFAULT)` macro registers each segment descriptor in a distributed slice
    at link time. On first use all segments are created sorted by name, so the segment index (the A2L
    `MEMORY_SEGMENT` number) is stable across runs regardless of creation order or threads, and is
    race-free. Previously, `CalSeg::new` created segments eagerly in call order, making the index
    non-deterministic.
  - Disabling the feature (`default-features = false`) makes `cal_seg!` fall back to eager creation in
    call order (equivalent to `CalSeg::new`); use this when all segments are created in a single,
    deterministic, race-free order.
  - Note: every crate that uses `cal_seg!` with the feature enabled must add `linkme` as a direct
    dependency (e.g. `linkme = "0.3"`).
  - Public API: new `cal_seg!` macro (re-exported at the crate root); `CalSeg::new` remains available
    for dynamic/`CalCell` use.
  - `calibration_demo` updated to use `cal_seg!`; documentation added to the root README and the
    calibration_demo README.


## [V3.0.0]

- New McRegisterType derive macro

New direct-registration system
xcp_register_type_derive proc-macro crate + McRegisterType trait/context in xcp_registry
Generated code registers types directly in the registry (no intermediate StructDescriptor/FieldDescriptor)
Migrated all call sites (XcpTypeDescription → McRegisterType, merged repeated #[characteristic(...)] attributes into one, string numbers → numeric literals):

All 8 examples (hello_xcp, calibration_demo, struct_measurement_demo, single/multi_thread_demo, rayon_demo, tokio_demo, point_cloud_demo) + added xcp_registry dependency to each
Removed the old xcp_type_description and xcp_type_description_derive crates completely.

Bumped all workspace versions 1.1.0 → 3.0.0 (root package + [workspace.package], xcp_registry, xcp_client, xcp_idl_generator + derive, all examples). tools left untouched.

Bug-Fixes:

- CalSeg has no Deref in this fork — two examples accessed params.<field> directly; changed to params.read_lock().<field>.
- rayon_demo used the removed CalSeg::sync() — replaced with manual PartialEq-based change detection.
- mc_address.rs test called add_a2l_cal_seg with the old 5-arg signature — added the missing number: None.
- Remaining warnings are pre-existing unused_assignments lints in the demos. 
