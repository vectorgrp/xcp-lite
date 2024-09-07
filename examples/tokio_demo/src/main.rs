// xcp-lite - tokio_demo
// Visualizes in CANape how tokio starts tasks in its worker threaad pool

mod xcp_server;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use core::f64::consts::PI;
use serde::{Deserialize, Serialize};
use std::error::Error;

use xcp::*;
use xcp_type_description::prelude::*;

//-----------------------------------------------------------------------------
// Demo calibration parameters (static)

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
    task1_cycle_time_us: 100000, // 100ms
    task2_cycle_time_us: 1000,   // 1ms
    task_count: 10,
});

//-----------------------------------------------------------------------------
// Demo calibration parameters (dynamic)

// Define a struct with calibration parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize, XcpTypeDescription)]
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
// This will be the FLASH page in the calibration memory segment
const CAL_PAGE1: CalPage1 = CalPage1 {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Asynchronous task, measures index, sleeps 100ms, measures -index and ends
// Demonstrates multi instance measurement
// There will be an event and an instance of index for each worker thread tokio uses
#[allow(dead_code)]
async fn task(task_index: u16) {
    let mut index: i16 = task_index as i16;

    trace!("task {} start", index);

    //let event = daq_create_event_instance!("task");
    //daq_register!(index, event, "Task index", "");
    // event.trigger();

    tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
    index = -index;

    //event.trigger();

    trace!("task {} end", index);
}

//-----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("xcp-lite tokio demo");

    // Initialize logger
    env_logger::Builder::new().filter_level(log::LevelFilter::Debug).init();

    // Start tokio XCP server
    let (rx_task, tx_task) = xcp_server::start_async_xcp_server("127.0.0.1:5555".to_string()).await?;
    let xcp = Xcp::get();

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
            break;
        }

        // Sleep for a calibratable amount of microseconds
        tokio::time::sleep(tokio::time::Duration::from_micros(calpage0.task1_cycle_time_us as u64)).await;

        // Start a number of asynchronous tasks and wait for them to finish
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

    info!("Stop");

    let _ = tokio::join!(rx_task);
    let _ = tokio::join!(tx_task);

    xcp.stop_server();
    Ok(())
}
