use xcp_type_description::prelude::*;

#[derive(Clone, Copy, XcpTypeDescription, Debug)]
struct Parent {
    #[comment = "Unique identifier"]
    #[min = "10"]
    #[max = "20"]
    #[unit = "unit"]
    uid: u32,
    child: Child,
    array: [f32; 16],
    map: [[i32; 9]; 1],
    ndim_array: [[[i32; 4]; 1]; 2],
}

#[derive(Clone, Copy, Debug, XcpTypeDescription)]
struct Child {
    uid: u32,
}

const PARENT: Parent = Parent {
    uid: 1,
    child: Child { uid: 2 },
    array: [
        0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5,
    ],
    map: [[0, 0, 0, 0, 0, 0, 0, 1, 2]],
    ndim_array: [[[1, 2, 3, 4]], [[13, 14, 15, 16]]],
};

fn main() {
    let chars = PARENT.characteristics();
    dbg!(chars);
}
