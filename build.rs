fn main() {
    build_info_build::build_script();

    #[allow(unused_assignments, unused_mut)] // due to feature flag
    let mut _is_posix = true;
    #[cfg(target_os = "windows")]
    {
        _is_posix = false;
    }

    #[allow(unused_assignments, unused_mut)] // due to feature flag
    let mut is_release = true;
    #[cfg(debug_assertions)]
    {
        is_release = false;
    }

    // Generate C code bindings for xcplib
    let bindings = bindgen::Builder::default()
        .header("xcplib/wrapper.h")
        //
        //.clang_args(&["-target", "x86_64-pc-windows-msvc"])
        .clang_arg("-Ixcplib/src")
        .clang_arg("-Ixcplib")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        //
        .blocklist_type("T_CLOCK_INFO")
        .allowlist_type("tXcpDaqLists")
        // Protocol layer
        .allowlist_function("XcpInit")
        .allowlist_function("XcpDisconnect")
        // Transport layer
        .allowlist_function("XcpEthTlGetInfo")
        // Server
        .allowlist_function("XcpEthServerInit")
        .allowlist_function("XcpEthServerShutdown")
        .allowlist_function("XcpEthServerStatus")
        // DAQ
        .allowlist_function("XcpEvent")
        .allowlist_function("XcpEventExt")
        //.allowlist_function("XcpTriggerDaqEventAt")
        //.allowlist_function("XcpEventAt")
        //.allowlist_function("XcpEventExtAt")
        // Misc
        .allowlist_function("XcpSetLogLevel")
        .allowlist_function("XcpPrint")
        .allowlist_function("XcpSetEpk")
        .allowlist_function("XcpSendTerminateSessionEvent")
        //.allowlist_function("ApplXcpGetAddr")
        .allowlist_function("ApplXcpSetA2lName")
        .allowlist_function("ApplXcpRegisterCallbacks")
        .allowlist_function("ApplXcpGetClock64")
        //
        .generate()
        .expect("Unable to generate bindings");
    bindings.write_to_file("src/xcp/xcplib.rs").expect("Couldn't write bindings!");

    // Build xcplib
    let mut builder = cc::Build::new();
    let builder = builder
        .include("xcplib/src/")
        .file("xcplib/src/xcpAppl.c")
        .file("xcplib/src/platform.c")
        .file("xcplib/src/xcpLite.c")
        .file("xcplib/src/xcpQueue64.c")
        .file("xcplib/src/xcpEthTl.c")
        .file("xcplib/src/xcpEthServer.c")
        .flag("-std=c11");

    if is_release {
        builder.flag("-O2");
    } else {
        builder.flag("-O0").flag("-g");
    }

    builder.compile("xcplib");

    // Tell cargo to invalidate the built crate whenever any of these files changed.
    println!("cargo:rerun-if-changed=xcplib/c_test.c");
    println!("cargo:rerun-if-changed=xcplib/wrapper.h");
    println!("cargo:rerun-if-changed=xcplib/src/main_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcptl_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcp_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/src/a2l.h");
    println!("cargo:rerun-if-changed=xcplib/src/a2l.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpAppl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpAppl.c");
    println!("cargo:rerun-if-changed=xcplib/src/platform.h");
    println!("cargo:rerun-if-changed=xcplib/src/platform.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpQueue.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpQueue64.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcp.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.c");
}
