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

    // Generate C code bindings for xcplib
    if is_posix {
        let bindings = bindgen::Builder::default()
            .header("xcplib/inc/xcplib.h")
            //
            //.clang_args(&["-target", "x86_64-pc-windows-msvc"])
            .clang_arg("-Ixcplib_cfg")
            .clang_arg("-Ixcplib/src")
            .clang_arg("-Ixcplib")
            // XCPlite configuration override for Rust xcp-lite
            .clang_arg("-DXCPLIB_CFG_OVERRIDE=\"xcplib_rust_cfg.h\"")
            .clang_arg("-DXCPLITE_CONFIGURATION=\"\"")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            //
            .blocklist_type("T_CLOCK_INFO")
            // Protocol layer
            .allowlist_function("XcpInit")
            .allowlist_function("XcpDeinit")
            .allowlist_function("XcpDisconnect")
            // Server
            // .allowlist_function("XcpEthServerGetInfo")
            .allowlist_function("XcpEthServerInit")
            .allowlist_function("XcpEthServerShutdown")
            .allowlist_function("XcpEthServerStatus")
            // CAL
            .allowlist_function("XcpCreateCalSeg")
            .allowlist_function("XcpFindCalSeg")
            .allowlist_function("XcpGetCalSegCount")
            .allowlist_function("XcpGetCalSegName")
            .allowlist_function("XcpGetCalSegSize")
            .allowlist_function("XcpGetCalSegNumber")
            .allowlist_function("XcpLockCalSeg")
            .allowlist_function("XcpUnlockCalSeg")
            // DAQ
            .allowlist_function("XcpEventExt")
            // Misc
            .allowlist_function("XcpSetLogLevel")
            .allowlist_function("XcpPrint")
            .allowlist_function("XcpSetA2lName")
            .allowlist_function("ApplXcpRegisterConnectCallback")
            .allowlist_function("XcpSendTerminateSessionEvent")
            .allowlist_function("ApplXcpGetClock64")
            //
            .generate()
            .expect("Unable to generate bindings");
        // Write the bindings only if they changed, build_info_build watches src/xcp/xcplib.rs and rewriting it would rerun this build script on every build
        let bindings = bindings.to_string();
        if std::fs::read_to_string("src/xcp/xcplib.rs").ok().as_deref() != Some(bindings.as_str()) {
            std::fs::write("src/xcp/xcplib.rs", bindings).expect("Couldn't write bindings!");
        }
    }

    // Build xcplib
    let mut builder = cc::Build::new();
    let builder = builder
        .include("xcplib_cfg/")
        .include("xcplib/inc/")
        .include("xcplib/src/")
        .file("xcplib/src/xcpappl.c")
        .file("xcplib/src/persistence.c")
        .file("xcplib/src/platform.c")
        .file("xcplib/src/sockets.c")
        .file("xcplib/src/xcplite.c")
        .file("xcplib/src/cal.c")
        .file("xcplib/src/queue64v.c")
        .file("xcplib/src/queue32.c")
        .file("xcplib/src/xcpethtl.c")
        .file("xcplib/src/xcpethserver.c");

    // XCPlite configuration override for Rust xcp-lite
    builder.define("XCPLIB_CFG_OVERRIDE", "\"xcplib_rust_cfg.h\"");

    if is_posix {
        builder.define("_GNU_SOURCE", None); // Required on Linux/GCC with -std=c11 for POSIX/GNU extensions (nanosleep, clock_gettime, CLOCK_MONOTONIC_RAW, pthread_mutexattr_settype, gmtime_r, ip_mreqn, ...)
        builder.flag("-std=c11");
        if is_release {
            builder.flag("-O2");
        } else {
            builder.flag("-O0").flag("-g");
        }
    }
    builder.compile("xcplib");

    // Tell cargo to invalidate the built crate whenever any of these files changed.
    println!("cargo:rerun-if-changed=xcplib_cfg/xcplib_rust_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/inc/xcplib.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcplib_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcptl_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcp_cfg.h");
    println!("cargo:rerun-if-changed=xcplib/inc/a2l.h");
    println!("cargo:rerun-if-changed=xcplib/src/a2l.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpappl.c");
    println!("cargo:rerun-if-changed=xcplib/src/platform.h");
    println!("cargo:rerun-if-changed=xcplib/src/platform.c");
    println!("cargo:rerun-if-changed=xcplib/src/sockets.h");
    println!("cargo:rerun-if-changed=xcplib/src/sockets.c");
    println!("cargo:rerun-if-changed=xcplib/src/queue.h");
    println!("cargo:rerun-if-changed=xcplib/src/queue64v.c");
    println!("cargo:rerun-if-changed=xcplib/src/queue32.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpethtl.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpethtl.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcpethserver.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcpethserver.c");
    println!("cargo:rerun-if-changed=xcplib/src/xcp.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcplite.h");
    println!("cargo:rerun-if-changed=xcplib/src/xcplite.c");
}
