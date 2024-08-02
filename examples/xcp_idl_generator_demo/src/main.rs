//TODO: Remove
#![allow(warnings)]

use xcp_idl_generator_derive::*;
use xcp::{IdlGenerator, IdlStructField, IdlStructFieldVec, IdlStruct, translate_idl_struct};

#[derive(IdlGenerator)]
struct Measurement {
    id: u32,
}

fn main() {
    let my_measurement = Measurement { id: 1 };
    let idl_gen = Measurement::generate_idl();
    let idl_str = translate_idl_struct(&idl_gen);
    dbg!(idl_str);
}
