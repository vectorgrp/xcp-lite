use cc::Build;

fn main() {
    build_info_build::build_script();

    // Generate XCPlite C code bindings
    // Uncomment this to regenerate the bindings

    let bindings = bindgen::Builder::default()
        .header("xcplib/wrapper.h")
        .clang_arg("-Ixcplib/src")
        .clang_arg("-Ixcplib")
        // Tell cargo to invalidate the built crate whenever any of the included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        //
        .blocklist_type("T_CLOCK_INFO")
        .allowlist_function("XcpInit")
        .allowlist_function("XcpEventExt")
        .allowlist_function("XcpPrint")
        .allowlist_function("XcpEthServerInit")
        .allowlist_function("XcpEthServerShutdown")
        .allowlist_function("XcpEthServerStatus")
        .allowlist_function("ApplXcpSetLogLevel")
        .allowlist_function("ApplXcpSetA2lName")
        .allowlist_function("ApplXcpRegisterCallbacks")
        //
        .generate()
        .expect("Unable to generate bindings");
    bindings
        .write_to_file("src/xcplite.rs")
        .expect("Couldn't write bindings!");

    // Build a XCP on ETH version of XCPlite as a library
    Build::new()
        .include("xcplib/src/")
        .include("xcplib/")
        .file("xcplib/xcpAppl.c")
        .file("xcplib/src/platform.c")
        .file("xcplib/src/xcpLite.c")
        .file("xcplib/src/xcpEthTl.c")
        .file("xcplib/src/xcpEthServer.c")
        .flag("-O2")
        .compile("xcplib");

    println!("cargo:rerun-if-changed=xcplib/wrapper.h");
    println!("cargo:rerun-if-changed=xcplib/main_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcptl_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcp_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/xcpAppl.c");
    println!("cargo:rerun-if-changed=xcplib/src/platform.h");
    println!("cargo:rerun-if-changed=xcplib/src/platform.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthTl.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpEthServer.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcp.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpLite.c");
}
