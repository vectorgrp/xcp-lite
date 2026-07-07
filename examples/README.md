# xcp-lite Examples

Each example is a standalone binary crate that instruments Rust code for XCP measurement and
calibration and ships with its own CANape project. This page collects the instructions that are
common to all examples — the individual example READMEs only describe what is specific to them.

## Building and running

Run any example from the workspace root:

```
cargo run -p <example_name>
```

Then connect with an XCP tool. A simple built-in test client is available in the XCPlite repo:

```
cargo run -p xcpclient -- --tcp
# or
cargo run -p xcpclient -- --udp
```

Alternatively, open the CANape project located in the example's `CANape/` folder.


## Common command line options

All examples share the same command line parser (crate `example_common`). Show the options of any
example with:

```
cargo run -p <example_name> -- --help
```

| Option              | Description                                                                                   |
| ------------------- | --------------------------------------------------------------------------------------------- |
| `-l, --log-level`   | Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5), default 3                        |
| `-b, --bind`        | Bind address, default is ANY (0.0.0.0)                                                         |
| `-t, --tcp`         | Use TCP as transport layer, default is UDP                                                     |
| `-p, --port`        | Port number, default 5555                                                                      |
| `-n, --name`        | Application name, defaults to the example name                                                 |
| `-f, --flatten`     | Flatten typedef structures into dot-mangled instance names in the A2L (for tools without TYPEDEF_STRUCTURE support); default writes typedefs |

## The examples

| Example | Description |
| ------- | ----------- |
| [all_features_demo](all_features_demo/README.md) | Comprehensive reference application that exercises the full feature surface in a single program. |
| [hello_xcp](hello_xcp/README.md) | A very basic example: measure a local variable and calibrate a parameter of basic scalar type. |
| [struct_measurement_demo](struct_measurement_demo/README.md) | Measurement of more complex types (struct, arrays of struct, multi-dimensional array slices). Generates A2L `TYPEDEF` and `INSTANCE` objects. |
| [calibration_demo](calibration_demo/README.md) | Various calibratable basic types, nested structs and multi-dimensional types with shared axis and lookup functions with interpolation. Generates A2L `CURVE` and `MAP` with shared `AXIS_PTS`. |
| [single_thread_demo](single_thread_demo/README.md) | Measure and calibrate in a single instance task thread; clone a calibration parameter set, move it to a thread and sync its calibration changes. |
| [multi_thread_demo](multi_thread_demo/README.md) | Measure and calibrate in a task instantiated in multiple threads with multiple instances of events and local variables. |
| [rayon_demo](rayon_demo/README.md) | Observe rayon workers calculating a mandelbrot set line by line. |
| [tokio_demo](tokio_demo/README.md) | Using XCP in an async tokio based application. |
| [point_cloud_demo](point_cloud_demo/README.md) | Measure a lidar point cloud and visualize it in CANape's 3D scene window using CDR serialization over XCP and the CDR/IDL schema generator proc-macro. |
