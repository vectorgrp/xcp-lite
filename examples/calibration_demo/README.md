# xcp-lite - calibration_demo

> See [the examples overview](../README.md) for common build, run and command line instructions.

Demonstrate various adjustable basic types, nested structs and multi dimensional User types such as a map based lookup table with shared axis and associated lookup functions with interpolation.
This generates A2L objects like CURVE and MAP with AXIS_PTS.

Note:
CANape and a2lfile do currently not support the THIS keyword. As a workaround, we have to specifiy fully qualified names referencing to individual instances  
  
Run:

```
cargo run -p calibration_demo
```

Start the CANape project in the CANape folder or find some screenshots below

## Creating calibration segments with `cal_seg!`

Calibration segments are created with the `cal_seg!` macro:

```rust
let calseg1 = cal_seg!("cal_demo_1", &CALPAGE1);
calseg1.register();
let calseg2 = cal_seg!("cal_demo_2", &CALPAGE2);
calseg2.register();
```

With the default `linkme` feature, `cal_seg!` registers each segment descriptor in a distributed
slice at link time. On first use all segments are created **sorted by name**, so the segment index
(the A2L `MEMORY_SEGMENT` number) stays stable across runs no matter the creation order or threads —
this is race-free and prevents unnecessary A2L churn. See the [Features](../../README.md#features)
section for details, including the requirement to add `linkme` as a direct dependency:

```toml
# Cargo.toml of any crate that uses cal_seg! (with the default linkme feature)
linkme = "0.3"
```

If you create all calibration segments in a single, deterministic, race-free order, you can disable
the feature (`default-features = false`); `cal_seg!` then falls back to eager creation in call order
(equivalent to `CalSeg::new`) and the `linkme` dependency is not needed.

## CANape

![CANape](CANape1.png)

![CANape](CANape2.png)

## A2L file

A measurement struct

```rust
// Struct measurement variable on stack
    #[derive(Clone, Copy, XcpTypeDescription)]
    struct Lookup {
        input: f32,
        output_linear: f32,
        output_spline: f32,
    }
    let mut lookup = /* Box<LookUp>::default() */Lookup {
        input: 0.0,
        output_linear: 0.0,
        output_spline: 0.0,
    };

    daq_register_struct!(lookup, event);
```

```
/* struct Lookup */
/begin TYPEDEF_MEASUREMENT Lookup.input "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1E32 1E32 /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT Lookup.output_linear "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1E32 1E32 /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT Lookup.output_spline "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1E32 1E32 /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_STRUCTURE Lookup "" 12 
 /begin STRUCTURE_COMPONENT input Lookup.input 0 /end STRUCTURE_COMPONENT
 /begin STRUCTURE_COMPONENT output_linear Lookup.output_linear 4 /end STRUCTURE_COMPONENT
 /begin STRUCTURE_COMPONENT output_spline Lookup.output_spline 8 /end STRUCTURE_COMPONENT
/end TYPEDEF_STRUCTURE

```

A user lookup table type as calibration map struct

```rust

#[derive(Clone, Copy, XcpTypeDescription)]
struct LookUpTable {
    #[axis(comment = "LookUpTable axis", min = "0", max = "10000")]
    input: [f32; 16],

    #[characteristic(comment = "LookUpTable values", axis = "CalPage.LookUpTable.input", min = "0", max = "10000")]
    output: [f32; 16],
}

// Default values for LookUpTable
impl Default for LookUpTable {
    fn default() -> Self { LookUpTable::DEFAULT }
}

// 'Class' LookUpTable
impl LookUpTable {
    const DEFAULT: LookUpTable = LookUpTable {
        input: [0.0, 1.0, 2.0, 5.0, 10.0, 220.0, 390.0, 730.0, 1000.0, 1880.0, 2770.0, 4110.0, 5000.0, 7010.0, 8640.0, 10000.0, ],
        output: [0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 530.0, 100.0, 610.0, 210.0, 980.0, 330.0, 730.0, 180.0, 350.0, 0.0],
    };

    fn new() -> Self { LookUpTable::DEFAULT }
    fn lookup(&self, input: f32) -> f32 { ... } 
}


```

```
/begin AXIS_PTS CalPage.LookUpTable.input "LookUpTable axis" 0x80010008 NO_INPUT_QUANTITY A_F32 0 NO_COMPU_METHOD 16 0 10000 /end AXIS_PTS
/begin CHARACTERISTIC CalPage.LookUpTable.output "LookUpTable values" CURVE 0x80010048 F32 0 NO_COMPU_METHOD 0 10000 
    /begin AXIS_DESCR COM_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  16 0.0 0.0 AXIS_PTS_REF CalPage.LookUpTable.input /end AXIS_DESCR 
/end CHARACTERISTIC

```
