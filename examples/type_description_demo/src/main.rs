use xcp::*;

#[derive(Clone, Copy, XcpTypeDescription, Debug)]
#[repr(C)]
struct FloatCalSeg {
    f1: f32,
    f2: f64,
    f3: f32,
    f4: f64,
}

#[derive(Clone, Copy, XcpTypeDescription, Debug)]
#[repr(C)]
struct IntCalSeg {
    i1: i32,
    i2: i16,
    i3: i64,
    i4: i8,
}

#[derive(Clone, Copy, XcpTypeDescription, Debug)]
#[repr(C)]
struct CombinedCalSeg {
    float_seg: FloatCalSeg,
    int_seg: IntCalSeg,
    array: [f32; 16],

    map: [[i32; 9]; 1],

    ndim_array: [[[i32; 4]; 1]; 2],
}

const CALSEG: CombinedCalSeg = CombinedCalSeg {
    float_seg: FloatCalSeg {
        f1: 1.0,
        f2: 2.0,
        f3: 3.0,
        f4: 4.0,
    },
    int_seg: IntCalSeg { i1: 1, i2: 2, i3: 3, i4: 4 },
    array: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
    map: [[0, 0, 0, 0, 0, 0, 0, 1, 2]],
    ndim_array: [[[1, 2, 3, 4]], [[13, 14, 15, 16]]],
};

fn main() {
    dbg!(CALSEG.type_description());
}
