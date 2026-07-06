# xcp_lite - hello_xcp

Demonstrates the usage of xcp-lite for Rust together with a CANape project.  

> See [the examples overview](../README.md) for common build, run and command line instructions.

Basic demo

Run:

```
cargo r -p hello_xcp
```

Run the test XCP client (from C/C++ XCPlite) in another terminal or start CANape with the project in folder examples/hello_xcp/CANape


```

xcpclient --udp --mea "counter" --verbose 2
xcpclient --udp  --upload-a2l --a2l tmp.a2l --list-cal '.*' --cal my_params.counter_max 10 --list-mea ".*"  --mea 'counter'


// Write a flattened A2L (typedefs expanded into mangled instance names)
cargo run -p hello_xcp -- --flatten


```

## CANape

![CANape](CANape.png)

