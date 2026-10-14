# CLAUDE.md

Guidance for AI coding agents working in this repository.

## What this is

`xcp_lite` (v3.0.10, edition 2024, rust-version 1.90.0) — a Rust measurement & calibration
library implementing the ASAM **XCP** protocol, wrapping the C `xcplib` (XCP server, Ethernet
transport). The Rust layer adds: a type **registry**, **A2L** file generation, a `CalSeg<T>`
calibration-segment wrapper, and measurement macros. Used with tools like Vector CANape.

## Repository layout

- Root crate `xcp_lite` — **library only** (no `src/main.rs`): `src/lib.rs`, `src/xcp/`
  (`mod.rs`, `cal.rs`, …), `src/metrics/`.
- Workspace members: `xcp_registry`, `xcp_register_type_derive`, `xcp_idl_generator`,
  `tests/support/xcp_test_client` (pkg `xcp_test_client`), `examples/common`
  (pkg `example_common`), and the example bins.
- Examples (each a bin with its own `CANape/` project + README): `all_features_demo`,
  `hello_xcp`, `calibration_demo`, `struct_measurement_demo`, `single_thread_demo`,
  `multi_thread_demo`, `rayon_demo`, `tokio_demo`, `point_cloud_demo`.
  (`examples/heap_demo` is a CANape folder only, not a crate.)
- `examples/README.md` is a hub with common run/CLI docs; per-example READMEs link back to it;
  the root README `## Examples` links to the hub.
- **`xcplib/` is a git SUBMODULE (separate repo) — do NOT edit** (including `tools/xcpclient`,
  `tools/bintool`).

## Key APIs

- `Xcp::init(app_name, app_revision, log_level)` → builder;
  `.start_server(tl, addr_octets, port, queue_size)` → `Result<&'static Xcp>`.
- `Xcp::get() -> &'static Xcp`; `xcp.set_registry_mode(flatten_typedefs, prefix_names)`;
  `xcp.finalize_registry()`; `xcp.stop_server()`; `xcp.check_server()`.
- A2L is written to the process CWD as `{app_name}.a2l` on `finalize_registry()` or on first
  client connect (lazy). Registry default mode = typedefs (`flatten = false`); flattening is an
  export-time transform on registry close.
- `CalSeg<T>`: `CalSeg::new(name, &DEFAULT)`, `.register()`, `.load("x.json")`,
  `.save("x.json")`, `.read_lock()`, `CalSeg::clone(&seg)` (Arc-like, `Send` not `Sync`,
  `Deref`). `T: Copy + McRegisterType` (+ serde for JSON).
- Measurement macros: `daq_create_event!`, `daq_create_event_tli!`, `daq_register!`,
  `daq_register_array!`, `daq_register_struct!`, `daq_capture!`, `daq_capture_tli!`,
  `event.trigger()`, `xcp_println!`.
- Shared example CLI: `example_common::ExampleArgs` (clap) — `log_level`, `bind`, `tcp`,
  `port`, `name: Option<String>`, `flatten`. Methods: `parse()`, `app_name(default)`,
  `init_logging()`.

## The McRegisterType derive macro

Derive `#[derive(McRegisterType)]` on calibration/measurement structs.

- Proc-macro crate: `xcp_register_type_derive` (`proc-macro = true`). It emits fully-qualified
  `::xcp_registry::…` paths as tokens and therefore does **not** depend on `xcp_registry` as a
  regular dependency (no dependency cycle) — it does have a `dev-dependency` on `xcp_registry`
  for its own `tests/` (see Build & test). The runtime trait `McRegisterType` + context live in
  `xcp_registry/src/mc_register_type.rs`.
- Source modules: `src/attr.rs` (field attribute parsing), `src/ty.rs` (type parsing/mapping),
  `src/enum_derive.rs` (`#[derive(McRegisterEnum)]`, see below), `src/lib.rs` (expansion).
- **Full design specification: `xcp_register_type_derive/DESIGN.md`** (authoritative reference;
  covers the typedef-only generation model, attribute syntax, and flattening as an export-time
  transform).
- Attributes: `#[characteristic(...)]`, `#[axis(...)]`, `#[measurement(...)]`. Keys map to
  `McSupportData` setters (comment/min/max/step/unit/factor/offset/qualifier/axis refs/
  input_quantity).
- **Enum fields**: two forms.
  - Preferred: `#[derive(McRegisterEnum)]` on the (fieldless, `#[repr(uN/iN)]`) enum itself —
    generates an `impl McEnumType` (value type + A2L verbal-conversion unit string from the
    variant names/discriminants). Then annotate the field with a bare `#[characteristic(enum_type)]`
    (no value); it defers to the enum's `McEnumType` impl at `<EnumType as McEnumType>::…`, so
    nothing is restated at the use site. Implemented in `enum_derive.rs`
    (`expand_enum`/`find_repr_int`/`eval_discriminant`).
  - Manual/legacy: `#[characteristic(enum_type = "u8", unit = "0 \"OFF\" 1 \"ON\" …")]` restates
    the backing int type and unit string by hand at every use site.
  - Either form treats the field as that integer scalar and skips typedef recursion (the derive
    on the containing struct cannot see the enum's `#[repr]`/size on its own). A compile-time
    `size_of` assertion checks **width** (not signedness). Accepted int names:
    `u8`/`u16`/`u32`/`u64`/`usize`/`i8`/`i16`/`i32`/`i64`/`isize`. Parsed in `attr.rs`
    (`FieldAttrs.enum_type`, `enum_auto`, `key_allowed`, `apply_key`), mapped in
    `ty.rs::enum_int_value_type_tokens`, used in `lib.rs` `expand()`.
- A2L writer (`xcp_registry/src/a2l/a2l_writer.rs`) auto-builds `COMPU_VTAB` + `COMPU_METHOD`
  from an enum-format `unit` string via `enum_pair_count`; the `phys_unit()` helper suppresses
  the invalid `PHYS_UNIT` for enum-format units.

## Build & test

- `cargo build -p <crate>`, `cargo build --workspace`, `cargo run -p <example>`.
- `cargo test -p xcp_registry`. Integration tests (`tests/test_single_thread.rs`,
  `tests/test_multi_thread.rs`) build their OWN A2L at runtime and use `xcp_test_client` +
  `tests/support/xcp_test_executor.rs`.
- `cargo test -p xcp_register_type_derive` runs `xcp_register_type_derive/tests/*.rs` (e.g.
  `type_group.rs`, a regression test for macro-generated field types being wrapped in
  `syn::Type::Group`) — these are ordinary integration tests exercising the derive end-to-end
  against the `xcp_registry` dev-dependency, not `trybuild`.
- Optional feature `a2l_reader` (via `xcp_registry/a2l_reader`) enables automatic A2L check
  using `a2lfile`. Default feature `linkme` gives deterministic, race-free calibration-segment
  indexing for `cal_seg!`.
- Pre-existing warnings to ignore: `xcplib` C unused-param; `unused_assignments` in
  `struct_measurement_demo`.
- macOS has no `timeout` — use a background PID + sleep + kill. `build-info-build` can throw a
  transient `GlobError` on `target/debug/deps/...`; just retry the build.

## Conventions & gotchas

- Root `Cargo.lock` **is** tracked — do not re-ignore it.
- `cfg(test)` is not active for dependency crates during integration tests.
- `add_typedef` (size) + `add_typedef_field` (offset/dim_type/size) in
  `xcp_registry/src/mc_registry.rs` validate structural typedef equality; the derive
  re-submits typedef fields after a duplicate typedef.
- `McDimType` derives `PartialEq`.
- Do not create markdown docs to describe changes unless asked. Comments: one short line, only
  for what the code cannot show.

## xcplib C layer — what the Rust wrapper reuses vs replaces

The Rust crate is a thin wrapper over `xcplib/` (a git submodule — **do not edit**). Key facts:

- **Reused from xcplib**: XCP protocol engine (`xcplite.c`), Ethernet server (`xcpethserver.c`), transport-layer lock-free queue (`queue64v.c`), calibration RCU (`cal.c`), clock, platform abstraction.
- **Replaced by Rust**: A2L generation (xcplib's generator is disabled via `#undef OPTION_ENABLE_A2L_GENERATOR` in `xcplib_cfg/xcplib_rust_cfg.h`; Rust uses `xcp_registry/src/a2l/a2l_writer.rs`). DAQ event management (xcplib's event list disabled via `#undef OPTION_DAQ_EVENT_LIST`; Rust has its own).
- **Rust-specific xcplib config**: `xcplib_cfg/xcplib_rust_cfg.h` — this file is the single source of truth for all xcplib compile-time options active in the Rust build. Read it before reasoning about feature flags.

Notable active options:
- `OPTION_CAL_SEGMENTS` — xcplib manages calibration segment pages.
- `OPTION_CAL_SEGMENT_EPK` — calibration segment index 0 is reserved for the EPK version string; `CalSeg` indices start at 1.
- `OPTION_CAL_SEGMENTS_ABS` is **not defined** — addressing is segment-relative (address extension 0 = segment-relative, encodes segment index in high word of address).
- `OPTION_QUEUE_64_VAR_SIZE` — 64-bit lock-free variable-size transmit queue; no locking on the DAQ producer path.
- `OPTION_ENABLE_PERSISTENCE` is **not defined** — no BIN file persistence; A2L is always regenerated at startup.

## Calibration RCU (`CalSeg<T>`)

`CalSeg<T>` in Rust wraps xcplib's 3-page RCU calibration implementation (`src/cal.c`). The design is documented in `xcplib/docs/CAL_RCU.md`. Critical invariants:

- **Exactly one writer**: the XCP command thread. Application threads are readers only.
- **Lock-free/wait-free reads**: `CalSeg::read_lock()` maps to `XcpLockCalSeg`/`XcpUnlockCalSeg` — no mutex, no allocation.
- **Visibility delay**: a calibration write becomes visible to readers at the *second* `XcpLockCalSeg` call after the write, not immediately.
- **Pages**: ecu\_page (current reader state), xcp\_page (writer state), free\_page (for swapping). Memory overhead: 64-byte header + 4× page size per segment.
- **Registration** (creating a segment) uses a mutex and is acceptable as a one-time cost; reads are always lock-free.

## DAQ measurement — hot-path cost

`event.trigger()` → `DaqTriggerEvent` in xcplib → lock-free enqueue into the 64-bit variable-size transmit queue. No mutex, no heap allocation. The first call to `daq_create_event!` / `daq_register!` does a lazy name lookup and caches the handle in a static/thread-local; subsequent calls are a direct handle use.

## EPK version string

`XcpSetEpk()` in xcplib (`src/xcplite.c`) sanitizes the EPK on storage: spaces, tabs, and colons are replaced with `_`. `Xcp::init()` pre-applies the same sanitization before passing to both `XcpInit()` and the registry, so the A2L and the value reported over XCP always match.



1. **Minimum application impact** — the XCP implementation must not perturb the user application: no locks, no heap allocations on the hot path (measurement capture, calibration reads).
2. **Ergonomic, idiomatic API** — user-facing APIs should be as natural and Rust-idiomatic as possible.
3. **Exceptions** — allocations and locks are acceptable only where unavoidable: one-time registration steps, `once`/`lazy_static` init patterns, and A2L/registry setup.

## Registry & McText lifetime model

- The registry is a process-singleton with effectively `'static` lifetime.
- `McText` holds `&'static str`. `From<String> for McText` intentionally **leaks** the `String` (via `Box::leak`) to obtain a `'static` reference. `From<&'static str> for McText` is zero-cost.
- Consequence: pass an owned `String` (not `.to_string()` on an already-owned value) to registry APIs so the single allocation is the one that gets leaked, not a copy of it.

## Agent workflow notes

- Prefer editing existing files over creating new ones.
- Read files before editing; make targeted edits with surrounding context.
- Don't edit the `xcplib/` submodule.
