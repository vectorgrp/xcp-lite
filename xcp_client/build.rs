fn main() {
    build_info_build::build_script();

    // Generate XCPlite C code bindings
    // Uncomment this to regenerate the bindings
    let bindings = bindgen::Builder::default()
        .header("../mdflib/wrapper.h")
        .clang_arg("-I../mdflib/src")
        .clang_arg("-I../mdflib")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        //
        .allowlist_function("mdfOpen")
        .allowlist_function("mdfCreateChannelGroup")
        .allowlist_function("mdfCreateChannel")
        .allowlist_function("mdfWriteHeader")
        .allowlist_function("mdfWriteRecord")
        .allowlist_function("mdfClose")
        //
        .generate()
        .expect("Unable to generate bindings");
    bindings.write_to_file("src/mdflib.rs").expect("Couldn't write bindings!");

    // Build a XCP on ETH version of XCPlite as a library
    cc::Build::new()
        .include("../mdflib/src/")
        .include("../mdflib/")
        .file("../mdflib/src/mdfWriter.c")
        .flag("-O2")
        .compile("mdflib");

    // Tell cargo to invalidate the built crate whenever any of these files changed.
    println!("cargo:rerun-if-changed=mdflib/wrapper.h");
    println!("cargo:rerun-if-changed=mdflib/src/main.h");
    println!("cargo:rerun-if-changed=mdflib/src/mdf4.h");
    println!("cargo:rerun-if-changed=mdflib/src/mdfWriter.h");
    println!("cargo:rerun-if-changed=mdflib/src/mdfWriter.c");
}
