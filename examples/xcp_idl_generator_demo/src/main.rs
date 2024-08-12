use std::fs::File;
use std::io::Write;
use xcp_idl_generator::prelude::*;

#[derive(IdlGenerator)]
struct Dummy {
    _vec: Vec<u32>,
}

#[derive(IdlGenerator)]
struct Measurement {
    _id: u32,
    _vec: Vec<Vec<u32>>,
    _dummy: Dummy,
    _dummy_vec: Vec<Dummy>,
}

fn write_string_to_file(filename: &str, content: &str) {
    let mut file = File::create(filename).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

fn main() {
    let description = Measurement::description();

    dbg!(&description);

    let idl_str = GeneratorCollection::generate(&IDL::CDR, &description).unwrap();
    println!("{}", idl_str);

    write_string_to_file("./gen.txt", &idl_str);
}
