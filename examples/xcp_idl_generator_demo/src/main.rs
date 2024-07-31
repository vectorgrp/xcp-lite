use xcp_idl_generator_derive::*;
use xcp::IdlGenerator;

#[derive(IdlGenerator)]
struct Measurement {
    id: u32,
}

fn main() {
    let my_measurement = Measurement { id: 1 };
    my_measurement.generate_idl();
}
