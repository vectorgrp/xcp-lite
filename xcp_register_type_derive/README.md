# xcp_register_type_derive

Proc-macro crate providing `#[derive(McRegisterType)]`.

The derive generates an `impl xcp_registry::McRegisterType for T` whose `register` method
calls the `xcp_registry` API directly (`add_typedef` / `add_typedef_field` / `add_instance`),
without any intermediate descriptor data structures.

The generated code emits fully-qualified `::xcp_registry::...` paths, so the consuming crate
must depend on `xcp_registry` directly. This crate itself does **not** depend on `xcp_registry`
(no dependency cycle), it only emits paths as tokens.

See the design specification in `../xcp_register_type/README.md`.
