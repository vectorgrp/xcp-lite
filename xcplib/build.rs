// Generates Xcp Lite C bindings and compiles the C library
fn main() {
    build_info_build::build_script();

    #[allow(unused_assignments, unused_mut)] // due to feature flag
    let mut is_posix = true;
    #[cfg(target_os = "windows")]
    {
        is_posix = false;
    }

    #[allow(unused_assignments, unused_mut)] // due to feature flag
    let mut is_release = true;
    #[cfg(debug_assertions)]
    {
        is_release = false;
    }

    // NOTE: set to true to regenerate the bindings
    // NOTE: binding generation on Windows is not maintained or tested and most likely will not work without installing libclang, etc.
    // => just stick to checked in bindings on Windows and regenerate them on a proper OS.
    const GENERATE_BINDINGS: bool = false;
    if GENERATE_BINDINGS {
        let bindings = bindgen::Builder::default()
            .header("wrapper.h")
            .clang_arg("-I.")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            //
            .blocklist_type("T_CLOCK_INFO")
            // Protocol layer
            .allowlist_function("XcpInit")
            .allowlist_function("XcpDisconnect")
            // ETH server
            .allowlist_function("XcpEthServerInit")
            .allowlist_function("XcpEthServerShutdown")
            .allowlist_function("XcpEthServerStatus")
            .allowlist_function("XcpEthServerGetInfo")
            // DAQ
            .allowlist_function("XcpEvent")
            .allowlist_function("XcpEventExt")
            .allowlist_function("XcpTriggerDaqEventAt")
            // Transmit Queue Access
            .allowlist_function("XcpTlInit")
            // Misc
            .allowlist_function("XcpPrint")
            .allowlist_function("ApplXcpSetLogLevel")
            .allowlist_function("ApplXcpSetA2lName")
            .allowlist_function("ApplXcpGetAddr")
            .allowlist_function("ApplXcpRegisterCallbacks")
            //
            .generate()
            .expect("Unable to generate bindings");

        bindings.write_to_file("src/lib.rs").expect("Couldn't write bindings!");
    }

    // Build a XCP on ETH version of XCPlite as a library
    let mut binding = cc::Build::new();
    let builder = binding
        .include(".")
        .file("src/xcpAppl.c")
        .file("src/platform.c")
        .file("src/queue.c")
        .file("src/xcpLite.c")
        .file("src/xcpTl.c");

    // Target specific sources
    if is_posix {
        builder.file("src/posix/xcpEthServer.c");
    } else {
        builder.file("src/windows/xcpEthServer.c");

        // xcplib requires a gnu compiler from mingw on Windows, msvc will most likely not work
        // => install mingw-w64 and GNU make using chocolatey
        builder.compiler("gcc");
    }

    // Flags
    builder.flag("-std=c11");
    if is_release {
        builder.flag("-O2");
    } else {
        builder.flag("-O0").flag("-g");
    }

    builder.compile("xcplib");

    // Tell cargo to invalidate the built crate whenever any of these files changed.
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=src/main_cfg.h");
    println!("cargo:rerun-if-changed=src/xcptl_cfg.h");
    println!("cargo:rerun-if-changed=src/xcp_cfg.h");
    println!("cargo:rerun-if-changed=src/xcpAppl.h");
    println!("cargo:rerun-if-changed=src/xcpAppl.c");
    println!("cargo:rerun-if-changed=src/platform.h");
    println!("cargo:rerun-if-changed=src/platform.c");
    println!("cargo:rerun-if-changed=src/xcpQueue.h");
    println!("cargo:rerun-if-changed=src/xcpQueue.c");
    println!("cargo:rerun-if-changed=src/xcpEthTl.h");
    println!("cargo:rerun-if-changed=src/xcpEthTl.c");
    println!("cargo:rerun-if-changed=src/xcpEthServer.h");
    println!("cargo:rerun-if-changed=src/xcpEthServer.c");
    println!("cargo:rerun-if-changed=src/xcp.h");
    println!("cargo:rerun-if-changed=src/xcpLite.h");
    println!("cargo:rerun-if-changed=src/xcpLite.c");
}
