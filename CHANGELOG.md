# Changelog

All notable changes to Rust xcp-lite are documented in this file.


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
