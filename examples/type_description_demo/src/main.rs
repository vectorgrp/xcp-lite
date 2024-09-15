//TODO: Remove
#[allow(warnings)]
use xcp::*;
use xcp_type_description::prelude::*;

#[derive(Clone, Copy, XcpTypeDescription, Debug)]
struct Parent {
    #[type_description(unit = "unit", min = "-1", max = "10.1", comment = "Parent comment")]
    uid: u32,

    child: Child,

    array: [f32; 16],

    map: [[i32; 9]; 1],

    ndim_array: [[[i32; 4]; 1]; 2],
}

#[derive(Clone, Copy, Debug, XcpTypeDescription)]
struct Child {
    #[type_description(comment = "child.uid")]
    uid: u32,
}

const PARENT: Parent = Parent {
    uid: 1,
    child: Child { uid: 2 },
    array: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    map: [[0, 0, 0, 0, 0, 0, 0, 1, 2]],
    ndim_array: [[[1, 2, 3, 4]], [[13, 14, 15, 16]]],
};

fn main() {
    let fields = PARENT.type_description();
    dbg!(fields);
}
