// hello_xcp
// xcp-lite basic demo
//
// Demonstrates the usage of xcp-lite for Rust together with a CANape project
//
// Run the demo
// cargo run -p hello_xcp
//
// Write a flattened A2L (typedefs expanded into mangled instance names)
// cargo run -p hello_xcp -- --flatten
//
// Run the test XCP client in another terminal or start CANape with the project in folder examples/hello_xcp/CANape
// xcp_client --udp --mea "counter" --verbose 2
// xcp_client --udp  --upload-a2l --a2l tmp.a2l --list-cal '.*' --cal my_params.counter_max 10 --list-mea ".*"  --mea 'counter'  --verbose 2

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "hello_xcp";
const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME: u32 = 10000; // 10ms

//-----------------------------------------------------------------------------
// Command line arguments (shared parser, see examples/common)

use example_common::ExampleArgs;

//-----------------------------------------------------------------------------
// Demo calibration parameters

// Define an enum
// Use an explicit `#[repr(..)]` so its integer width matches the `enum_type` attribute below.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum State {
    Off = 0,
    On = 1,
    STANDBY = 2,
}

// Define a struct with semantic annotations used as nested calibration parameter type
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
struct Point {
    #[characteristic(comment = "x coordinate", min = -100, max = 100)]
    x: f32,
    #[characteristic(comment = "y coordinate", min = -100.0, max = 100.0)]
    y: f32,
}

// Define calibration parameters in a struct with semantic annotations to create the A2L file
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
struct Params {
    // Bool
    #[characteristic(comment = "Demo bool, Start/stop counter")]
    counter_on: bool,

    // Integer
    #[characteristic(comment = "Demo u32, Max counter value", min = 0, max = 1023)]
    counter_max: u32,

    // Integer with physical conversion factor and offset
    #[characteristic(
        comment = "Demo u32, Task delay time in s, ecu internal value as u32 in us",
        min = 0.00001,
        max = 2,
        unit = "s",
        factor = 0.000001
    )]
    delay: u32,

    // Enum
    // The enum is treated as its integer representation (`enum_type`); the labels are described
    // by the `unit` string.
    #[characteristic(comment = "Demo enum", enum_type = "u8", unit = r#"0 "OFF" 1 "ON" 2 "STANDBY""#)]
    enum_field: State,

    // Arrays
    // More than 2 array dimensions is not supported by the derive macro
    #[characteristic(comment = "Demo array", min = 0, max = 100, axis = "array_axis")]
    array: [u8; 4],
    #[axis(comment = "Demo axis", min = 0, max = 100)]
    array_axis: [u8; 4],

    #[characteristic(comment = "Demo matrix", min = 0, max = 100)]
    matrix: [[u8; 8]; 4],

    // Nested structs
    #[characteristic(comment = "Demo struct")]
    struct_field: Point,

    // Array of structs
    // More than 2 array dimensions is not supported by the derive macro
    #[characteristic(comment = "Demo array of structs")]
    struct_array_field: [Point; 2],
}

// Default values for the calibration parameters
const PARAMS: Params = Params {
    counter_on: true,
    counter_max: 100,
    delay: MAINLOOP_CYCLE_TIME,
    enum_field: State::Off,
    array: [10, 11, 12, 13],
    array_axis: [0, 1, 2, 3],
    matrix: [
        [0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7],
        [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17],
        [0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27],
        [0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37],
    ],
    struct_field: Point { x: 1.0, y: 2.0 },
    struct_array_field: [Point { x: 1.0, y: 2.0 }, Point { x: 3.0, y: 4.0 }],
};

//-----------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    println!("XCP for Rust demo - hello_xcp - CANape project in ./examples/hello_xcp/CANape");

    // Args
    let args = ExampleArgs::parse();
    args.init_logging();

    // XCP: Initialize the XCP server
    let app_name = args.app_name(APP_NAME);
    let app_revision = build_info::format!("Version_{}", $.timestamp);
    let _xcp = Xcp::init(app_name, app_revision, args.log_level).start_server(
        if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
        args.bind.octets(),
        args.port,
        XCP_QUEUE_SIZE,
    )?;

    // XCP: Create a calibration segment wrapper with default values and register the calibration parameters
    let params = CalSeg::new("my_params", &PARAMS);
    params.register();

    // Demo measurement variable on stack
    let mut counter: u32 = 0;

    // XCP: Register a measurement event and bind measurement variables
    let event = daq_create_event!("my_event", 16);
    daq_register!(counter, event);

    // XCP: Choose the A2L representation for structs and arrays of structs.
    // Default (false) keeps TYPEDEF_STRUCTUREs; --flatten expands them into dot-mangled
    // leaf instances (e.g. struct_array_field._0.x) for tools without typedef support.
    _xcp.set_registry_mode(args.flatten, false);
    if args.flatten {
        info!("A2L will be written flattened (typedefs expanded into mangled instance names)");
    } else {
        info!("A2L will be written with typedef structures");
    }

    // @@@@ Test: create A2L file now, otherwise it will be created on first client connection
    _xcp.finalize_registry()?;

    let mut sleep_time: u64;
    loop {
        // XCP: Synchronize calibration parameters in cal_page and lock read access for consistency
        {
            let params = params.read_lock();

            if params.counter_on {
                counter += 1;
                if counter > params.counter_max {
                    counter = 0;
                }
            }

            sleep_time = params.delay as u64;
        }

        // XCP: Trigger timestamped measurement data acquisition
        event.trigger();

        std::thread::sleep(std::time::Duration::from_micros(sleep_time));
    }
}
