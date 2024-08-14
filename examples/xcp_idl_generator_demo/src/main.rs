use std::fs::{remove_file, File};
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

impl Measurement {
    fn new() -> Self {
        Self {
            _id: 0,
            _vec: vec![vec![0]],
            _dummy: Dummy { _vec: vec![0] },
            _dummy_vec: vec![Dummy { _vec: vec![0] }],
        }
    }
}

fn write_string_to_file(filename: &str, content: &str) {
    let _ = remove_file(filename);
    let mut file = File::create(filename).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

fn main() {
    let measurement = Measurement::new();
    let description = measurement.description();

    let val = GeneratorCollection::generate(&IDL::CDR, &description).unwrap();
    write_string_to_file("./gen.txt", &val);
}
