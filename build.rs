fn main() {
    build_info_build::build_script();

    // Generate XCPlite C code bindings

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
        // ETH server mode
        .allowlist_function("XcpEthServerInit")
        .allowlist_function("XcpEthServerShutdown")
        .allowlist_function("XcpEthServerStatus")
        .allowlist_function("XcpEthTlGetInfo")
        // DAQ
        .allowlist_function("XcpTriggerDaqEventAt")
        //.allowlist_function("XcpEventAt")
        .allowlist_function("XcpEvent")
        //.allowlist_function("XcpEventExtAt")
        .allowlist_function("XcpEventExt")
        // Misc
        .allowlist_function("XcpPrint")
        .allowlist_function("XcpSendTerminateSessionEvent")
        .allowlist_function("ApplXcpSetLogLevel")
        .allowlist_function("ApplXcpSetA2lName")
        .allowlist_function("ApplXcpSetEpk")
        .allowlist_function("ApplXcpGetAddr")
        .allowlist_function("ApplXcpRegisterCallbacks")
        //
        .generate()
        .expect("Unable to generate bindings");
    bindings.write_to_file("src/xcp/xcplib.rs").expect("Couldn't write bindings!");

    // Build a XCP on ETH version of XCPlite as a library
    cc::Build::new()
        .include("xcplib/src/")
        .include("xcplib/")
        .file("xcplib/xcpAppl.c")
        .file("xcplib/src/platform.c")
        .file("xcplib/src/xcpLite.c")
        .file("xcplib/src/xcpQueue.c")
        .file("xcplib/src/xcpTl.c")
        .file("xcplib/src/xcpEthTl.c")
        .file("xcplib/src/xcpEthServer.c")
        // C11
        .flag("-std=c11")
        // Optimizations
        // @@@@ Enable optimization in release mode
        .flag("-O2")
        // .flag("-O0")
        // .flag("-g")
        .compile("xcplib");

    // Tell cargo to invalidate the built crate whenever any of these files changed.
    println!("cargo:rerun-if-changed=xcplib/wrapper.h");
    println!("cargo:rerun-if-changed=xcplib/main_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcptl_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcp_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcpAppl.c");
    println!("cargo:rerun-if-changed=xcplib/src/main.h");
    println!("cargo:rerun-if-changed=xcplib/src/platform.h");
    println!("cargo:rerun-if-changed=xcplib/src/platform.c");
    println!("cargo:rerun-if-changed=xcplib/src/Queue.h");
    println!("cargo:rerun-if-changed=xcplib/src/Queue.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpTl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpTl.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcp.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.c");
}
