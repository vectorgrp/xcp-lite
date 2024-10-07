// xcp-lite - tokio_demo
// Implement the XCP server as a tokio task (xcp_server::xcp_task), no threads running in xcplib anymore
// Demo the usual measurement and calibration operations
// Demo how to visualize tokio tasks start/stop in tokios worker thread pool

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use core::f64::consts::PI;
use std::error::Error;

use xcp::*;
use xcp_type_description::prelude::*;

include!("../../../tests/xcp_server_task.rs");

//-----------------------------------------------------------------------------
// Demo calibration parameters (static)
// Does not create memory segments in A2L, manually added characterics in group "cal"

struct CalPage {
    run: bool,
}

static CAL_PAGE: once_cell::sync::OnceCell<CalPage> = once_cell::sync::OnceCell::with_value(CalPage { run: true });

struct CalPage0 {
    task1_cycle_time_us: u32, // Cycle time of task1 in microseconds
    task2_cycle_time_us: u32, // Cycle time of task2 in microseconds
    task_count: u16,          // Number of tasks
}

static CAL_PAGE0: once_cell::sync::OnceCell<CalPage0> = once_cell::sync::OnceCell::with_value(CalPage0 {
    task1_cycle_time_us: 10000, // 10ms
    task2_cycle_time_us: 1000,  // 1ms
    task_count: 10,
});

//-----------------------------------------------------------------------------
// Demo calibration parameters (dynamic, with auto A2L generation derive and attribute macros)
// Creates a memory segment "CalPage1" with 2 pages, a default "FLASH" page and a mutable "RAM" page

// Define a struct with calibration parameters
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Amplitude of the sine signal")]
    #[type_description(unit = "Volt")]
    #[type_description(min = "0")]
    #[type_description(max = "500")]
    ampl: f64,

    #[type_description(comment = "Period of the sine signal")]
    #[type_description(unit = "s")]
    #[type_description(min = "0.001")]
    #[type_description(max = "10")]
    period: f64,

    #[type_description(comment = "Counter maximum value")]
    #[type_description(min = "0")]
    #[type_description(max = "255")]
    counter_max: u32,
}

// Default calibration values
// This will be the read only default (FLASH) page in the calibration memory segment CalPage1
const CAL_PAGE1: CalPage1 = CalPage1 {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Experimental
// Asynchronous task, trigger measurement of local variable index, sleep 200us, measure -index and stop
// Demonstrates multi instance measurement
// There will be an event instance and an instance of variable index for each worker thread tokio uses
// Note:
// Once the A2L registry is created on XCP client connect, the event and variable instances are fixed and addional instances are not visible
// Tokio occasionally creates new worker threads and destroys old ones very late, so the number of instances may change
// Check what happens, when increasing/decreasing calpage0.task_count
#[allow(dead_code)]
async fn task(task_index: u16) {
    let mut index: i16 = task_index as i16;

    trace!("task {} start", index);

    let event = daq_create_event_tli!("task");
    daq_register!(index, event, "Task index", "");
    event.trigger();

    tokio::time::sleep(tokio::time::Duration::from_micros(200)).await;
    index = -index;

    event.trigger();

    trace!("task {} end", index);
}

//-----------------------------------------------------------------------------
// Main

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("xcp-lite tokio demo");

    // Initialize logger
    env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();

    // Start tokio XCP server
    // Initialize the xcplib transport and protocol layer only, not the server
    let xcp: &'static Xcp = XcpBuilder::new("tokio_demo").set_log_level(XcpLogLevel::Debug).tl_start().unwrap();
    let xcp_task = tokio::spawn(xcp_task(xcp, [127, 0, 0, 1], 5555));

    // let mut xcp_server = xcp_server::XcpServer::new([127, 0, 0, 1], 5555);
    // let xcp = xcp_server.start_xcp(xcp).await?;

    // Create and register a static calibration parameter set
    let calpage = CAL_PAGE.get().unwrap();
    cal_register_static!(calpage.run, "stop maintask");
    let calpage0 = CAL_PAGE0.get().unwrap();
    cal_register_static!(calpage0.task1_cycle_time_us, "task1 cycle time", "us");
    cal_register_static!(calpage0.task2_cycle_time_us, "task2 cycle time", "us");
    cal_register_static!(calpage0.task_count, "task count");

    // Create and register a calibration parameter set "calseg"
    // This will define a MEMORY_SEGMENT named "calseg" in A2L
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched during runtime (XCP set_cal_page), saved to json (XCP freeze) freeze, reinitialized from FLASH (XCP copy_cal_page)
    // The RAM page can be reloaded from a json file (load_json==true)
    // If A2L is enabled (enable_a2l), the A2L description will be generated and provided for upload by CANape
    let calseg = xcp.create_calseg(
        "CalPage1", // name of the calibration segment and the .json file
        &CAL_PAGE1, // default calibration values
        true,       // load RAM page from file "cal_seg".json
    );

    // Mainloop
    trace!("Start");
    let start_time = tokio::time::Instant::now();

    // Measurement variable
    let mut counter: u32 = 0;
    let mut channel_1: f64 = 0.0;

    // Create a measurement event with a unique name "task"
    // This will apear as measurement mode in the CANape measurement configuration
    let event = daq_create_event!("task");

    // Register local variables "counter" and "channel_1" and associate them to event "task"
    daq_register!(counter, event);
    daq_register!(channel_1, event, "sine wave signal", "Volt");

    // Main task loop
    loop {
        // Stop
        if !calpage.run {
            info!("mainloop stopped by calpage.run=false");
            break;
        }

        // Sleep for a calibratable amount of microseconds
        tokio::time::sleep(tokio::time::Duration::from_micros(calpage0.task1_cycle_time_us as u64)).await;

        // Start a number of short running asynchronous tasks and wait for them to finish
        let mut tasks = Vec::new();
        for i in 1..=calpage0.task_count {
            tasks.push(tokio::spawn(task(i)));
        }
        for t in tasks {
            let _ = tokio::join!(t);
        }

        // A saw tooth counter with max from a calibration parameter
        counter += 1;
        if counter > calseg.counter_max {
            counter = 0
        }

        // A sine signal with amplitude and period from calibration parameters
        let time = start_time.elapsed().as_micros() as f64 * 0.000001; // s
        channel_1 = calseg.ampl * (PI * time / calseg.period).sin();
        let _channel_2 = channel_1;

        // Triger the measurement event "task"
        // The measurement event timestamp is taken here and captured data is sent to CANape
        event.trigger();

        // Synchronize calibration operations, if there are any
        // All calibration (mutation of calseg) actions (download, page switch, freeze, init) on segment "calseg" happen here
        calseg.sync();
    }

    info!("mainloop stopped");

    xcp_task.abort();
    xcp.tl_shutdown();
    //xcp_task.await.unwrap()?;

    Ok(())
}
