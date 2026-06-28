// xcp-lite - tokio_demo

// Demo the usual measurement and calibration operations in an async environment
// Demo how to visualize tokio tasks start/stop in a tokio worker thread pool

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use core::f64::consts::PI;
use std::error::Error;

use xcp_lite::registry::*;
use xcp_lite::*;

//-----------------------------------------------------------------------------
// Parameters

const APP_NAME: &str = "tokio_demo";

const XCP_QUEUE_SIZE: u32 = 1024 * 64; // 64kB
const MAINLOOP_CYCLE_TIME_US: u32 = 1000; // 1ms

//-----------------------------------------------------------------------------
// Command line arguments (shared parser, see examples/common)

use example_common::ExampleArgs;

//-----------------------------------------------------------------------------
// Demo calibration parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, McRegisterType)]
struct CalPage1 {
    #[characteristic(comment = "Amplitude of the sine signal", unit = "Volt", min = 0, max = 500)]
    ampl: f64,

    #[characteristic(comment = "Period of the sine signal", unit = "s", min = 0.001, max = 10)]
    period: f64,

    #[characteristic(comment = "Counter maximum value", min = 0, max = 255)]
    counter_max: u32,
}

// Default calibration values
// This will be the read only default (FLASH) page in the calibration memory segment CalPage1
const CALPAGE1: CalPage1 = CalPage1 {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Experimental
// Asynchronous task, trigger measurement of local variables sine and index
// The event instance is created in the task function
// Stop after 5s
// Trigger a global event when any task starts or stops
// Trigger a thread local event, in each loop
// Once the A2L registry is created on XCP client connect, tli events and variable instances are fixed and additional instances are not visible
// Tokio occasionally creates new worker threads and destroys old ones very late, so the number of instances may change

#[allow(dead_code)]
async fn task(task_index: u32, calseg1: CalSeg<CalPage1>) {
    info!("task {} start", task_index);
    let start_time_instant = tokio::time::Instant::now();

    // Create a static event instance for this task
    let start_time: u64 = start_time_instant.elapsed().as_micros() as u64;
    let mut stop_time: u64 = 0;
    let index = task_index;
    let start_stop_event = daq_create_event!("start_task");
    daq_register!(index, start_stop_event, "Index of task", "");
    daq_register!(start_time, start_stop_event, "Start time of task", "");
    daq_register!(stop_time, start_stop_event, "Stop time of task", "");
    start_stop_event.trigger();

    // Create thread local event instances for this task
    // The number of events depend on the number of worker threads tokio uses
    let event = daq_create_event_tli!("tokio_task");

    // Register thread local variables
    let mut sine: f64 = 0.0;
    daq_register_tli!(sine, event, "Worker thread local instance of sine", "");

    loop {
        // A sine signal with amplitude and period from calibration parameters
        let time = start_time_instant.elapsed().as_micros() as f64 * 0.000001; // s
        let (ampl, period) = {
            let params = calseg1.read_lock();
            (params.ampl, params.period)
        };
        sine = ampl * (PI * time / period).sin();
        let _ = sine;

        event.trigger();

        tokio::time::sleep(tokio::time::Duration::from_micros(200)).await;

        event.trigger();

        // Stop after 5s
        if time > 5.0 {
            break;
        }
    }

    info!("task {} stop", task_index);
    stop_time = start_time_instant.elapsed().as_micros() as u64;
    let _ = stop_time;
    start_stop_event.trigger();
}

//-----------------------------------------------------------------------------
// Main

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("tokio demo");

    // Args
    let args = ExampleArgs::parse();
    args.init_logging();

    // XCP: Initialize the XCP server
    let app_name = args.app_name(APP_NAME);
    let app_revision = build_info::format!("{}", $.timestamp);
    let _ = Xcp::init(app_name, app_revision, args.log_level).start_server(
        if args.tcp { XcpTransportLayer::Tcp } else { XcpTransportLayer::Udp },
        args.bind.octets(),
        args.port,
        XCP_QUEUE_SIZE,
    )?;

    // XCP: Select flattened or typedef A2L representation (--flatten)
    Xcp::get().set_registry_mode(args.flatten, false);

    // Create and register calibration parameter sets
    // This will define a MEMORY_SEGMENT named "params" in A2L
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    // If A2L is enabled (enable_a2l), the A2L description will be generated and provided for upload by CANape
    let params = CalSeg::new("params", &CALPAGE1);
    params.register();

    // Mainloop
    info!("Start mainloop");

    // Create a measurement event
    // This will appear as measurement mode in the CANape measurement configuration
    let event = daq_create_event!("task");

    // Register local variables "counter" and "channel_1" and associate them to event "task"
    let mut counter: u32 = 0;
    let mut task_index: u32 = 0;
    daq_register!(task_index, event, "Index of next task to start", "");
    daq_register!(counter, event, "Demo variable counter", "");

    // Main task loop
    loop {
        // Start tasks randomly
        // Generate a random number r as double between 0.0 and 1.0 and start a new task with probability 0.1%
        // The tasks will terminate after 5s
        let r: f64 = rand::random();
        if r > 0.998 {
            task_index += 1;
            let calseg = CalSeg::clone(&params);
            tokio::spawn(async move {
                let _ = task(task_index, calseg).await;
            });
        }

        // Sleep for 1ms
        tokio::time::sleep(tokio::time::Duration::from_micros(MAINLOOP_CYCLE_TIME_US as u64)).await;

        // A saw tooth counter with max from a calibration parameter
        counter += 1;
        if counter > params.read_lock().counter_max {
            counter = 0;
        }

        // Trigger the measurement event "task"
        // The measurement event timestamp is taken here and captured data is sent to CANape
        event.trigger();
    }

    // for t in tasks {
    //     let _ = tokio::join!(t);
    // }

    // Stop the XCP server
    // xcp.stop_server();
    // Ok(())
}
