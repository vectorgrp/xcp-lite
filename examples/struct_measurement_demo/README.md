# xcp_lite - struct_measurement_demo

Demonstrate measurement of nested struct instances  
Make use of A2L objects INSTANCE, TYPEDEF_MEASUREMENT TYPEDEF_STRUCTURE and STRUCTURE_COMPONENT  

Run:
```
cargo run --example struct_measurement_demo  
```

Start the CANape project in the CANape folder or find some screenshots below  



## CANape
 

![CANape](CANape1.png)


![CANape](CANape2.png)


![CANape](CANape3.png)


![CANape](CANape4.png)


## A2L file 

Code:

``` rust

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Counters {
    #[measurement(comment = "counter", min = "0.0", max = "1000.0")]
    a: i16,
    #[measurement(comment = "counter*2", min = "0.0", max = "2000.0")]
    b: u64,
    #[measurement(comment = "counter*3", min = "0.0", max = "3000.0")]
    c: f64,
}

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Point {
    #[measurement(comment = "x-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    x: f32,
    #[measurement(comment = "y-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    y: f32,
    #[measurement(comment = "z-coordinate", min = "-10.0", max = "10.0", unit = "m")]
    z: f32,
}

#[derive(XcpTypeDescription, Debug, Clone, Copy)]
struct Data {

    // Scalar value
    cycle_counter: u32, 

    // Scalar values with annotations for min, max, conversion rule, physical unit, ...
    #[measurement(comment = "cpu temperature in grad celcius", min = "-50", max = "150", offset = "-50.0", unit = "deg/celcius")]
    cpu_temperature: u8,
    #[measurement(comment = "mainloop cycle time in s, converted from us", factor = "0.000001", unit = "s")]
    cycle_time: u32,

    // Array of scalar value with conversion rule
    #[measurement(comment = "cycle_time distribution in %", min = "0", max = "100", unit = "%")]
    cycle_time_distribution: [u32; 100],

    // Single instance of a point
    #[measurement(comment = "A single point")]
    point: Point,

    // An array of points
    #[measurement(comment = "Array of 8 points")]
    point_array: [Point; 8],

    #[measurement(comment = "Matrix of 16*16 float values")]
    float_matrix: [[f32; 32]; 32],
}


```


Generated A2L:

```

/* Typedefs */
/begin TYPEDEF_MEASUREMENT a "counter" SWORD IDENTITY 0 0 0 1000  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT b "counter*2" A_UINT64 IDENTITY 0 0 0 2000  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT c "counter*3" FLOAT64_IEEE NO_COMPU_METHOD 0 0 0 3000  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_STRUCTURE Counters "" 24
	/begin STRUCTURE_COMPONENT a a 16 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT b b 0 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT c c 8 /end STRUCTURE_COMPONENT
/end TYPEDEF_STRUCTURE
/begin TYPEDEF_MEASUREMENT cycle_counter "" ULONG IDENTITY 0 0 0 4294967295  /end TYPEDEF_MEASUREMENT
/begin COMPU_METHOD cpu_temperature "" LINEAR "%.0" "deg/celcius" COEFFS_LINEAR 1 -50 /end COMPU_METHOD
/begin TYPEDEF_MEASUREMENT cpu_temperature "cpu temperature in grad celcius" UBYTE cpu_temperature 0 0 -50 150  /end TYPEDEF_MEASUREMENT
/begin COMPU_METHOD cycle_time "" LINEAR "%.6" "s" COEFFS_LINEAR 0.000001 0 /end COMPU_METHOD
/begin TYPEDEF_MEASUREMENT cycle_time "mainloop cycle time in s, converted from us" ULONG cycle_time 0 0 0 4294.9672949999995  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT cycle_time_distribution "cycle_time distribution in %" ULONG IDENTITY 0 0 0 100  MATRIX_DIM 100 /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT x "x-coordinate" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -10 10  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT y "y-coordinate" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -10 10  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_MEASUREMENT z "z-coordinate" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -10 10  /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_STRUCTURE Point "" 12
	/begin STRUCTURE_COMPONENT x x 0 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT y y 4 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT z z 8 /end STRUCTURE_COMPONENT
/end TYPEDEF_STRUCTURE
/begin TYPEDEF_MEASUREMENT float_matrix "Matrix of 16*16 float values" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -100000000000000000000000000000000 100000000000000000000000000000000  MATRIX_DIM 32 32 /end TYPEDEF_MEASUREMENT
/begin TYPEDEF_STRUCTURE Data "" 4616
	/begin STRUCTURE_COMPONENT cycle_counter cycle_counter 4592 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT cpu_temperature cpu_temperature 4612 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT cycle_time cycle_time 4596 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT cycle_time_distribution cycle_time_distribution 4192 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT point Point 4600 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT point_array Point 4096 MATRIX_DIM 8 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT float_matrix float_matrix 0 /end STRUCTURE_COMPONENT
/end TYPEDEF_STRUCTURE
/begin TYPEDEF_CHARACTERISTIC mainloop_cycle_time "Cycle time of the mainloop" VALUE U32 0 IDENTITY 100 10000  /end TYPEDEF_CHARACTERISTIC
/begin TYPEDEF_CHARACTERISTIC counter_max "Counter wraparound" VALUE U16 0 IDENTITY 0 10000  /end TYPEDEF_CHARACTERISTIC
/begin TYPEDEF_CHARACTERISTIC ampl "Amplitude of the sine signal" VALUE F64 0 NO_COMPU_METHOD 0 500  /end TYPEDEF_CHARACTERISTIC
/begin TYPEDEF_CHARACTERISTIC period "Period of the sine signal" VALUE F64 0 NO_COMPU_METHOD 0.001 10  /end TYPEDEF_CHARACTERISTIC
/begin TYPEDEF_STRUCTURE Parameters "" 24
	/begin STRUCTURE_COMPONENT mainloop_cycle_time mainloop_cycle_time 16 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT counter_max counter_max 20 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT ampl ampl 0 /end STRUCTURE_COMPONENT
	/begin STRUCTURE_COMPONENT period period 8 /end STRUCTURE_COMPONENT
/end TYPEDEF_STRUCTURE


/* Measurements */
/begin MEASUREMENT counter1 "" A_UINT64 IDENTITY 0 0 0 18446744073709552000 ECU_ADDRESS 0x3C ECU_ADDRESS_EXTENSION 3 /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0 /end DAQ_EVENT /end IF_DATA /end MEASUREMENT
/begin MEASUREMENT counter2 "" ULONG IDENTITY 0 0 0 4294967295 ECU_ADDRESS 0x48 ECU_ADDRESS_EXTENSION 3 /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0 /end DAQ_EVENT /end IF_DATA /end MEASUREMENT
/begin INSTANCE counters "" Counters 0xFFFFFFD4 ECU_ADDRESS_EXTENSION 3 /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0 /end DAQ_EVENT /end IF_DATA /end INSTANCE
/begin INSTANCE data "" Data 0xFFFFEDA4 ECU_ADDRESS_EXTENSION 3 /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT 0 /end DAQ_EVENT /end IF_DATA /end INSTANCE
/begin GROUP Measurements "" ROOT /begin SUB_GROUP mainloop /end SUB_GROUP /end GROUP
/begin GROUP mainloop "" /begin REF_MEASUREMENT counter1 counter2 counters data /end REF_MEASUREMENT /end GROUP

/* Axis */

/* Characteristics */
/begin INSTANCE parameters "" Parameters 0x80010000 /end INSTANCE


```



 
