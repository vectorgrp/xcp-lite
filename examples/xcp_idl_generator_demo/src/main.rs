use xcp_idl_generator::prelude::*;
use std::fs::File;
use std::io::Write;

#[derive(IdlGenerator)]
struct Measurement {
    _id: u32,
}

fn write_string_to_file(filename: &str, content: &str) {
    let mut file = File::create(filename).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

fn main() {
    let description = Measurement::description();
    let idl_str = generate(IDL::CDR, &description);
    println!("{}", idl_str);
    write_string_to_file("./gen.txt", &idl_str);
}
