// xcp-lite - multi_thread_demo

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use std::{
    f64::consts::PI,
    fmt::Debug,
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;

use xcp::*;

// Static application start time
lazy_static::lazy_static! {
    static ref START_TIME: Instant = Instant::now();
}

//-----------------------------------------------------------------------------
// Demo calibration parameters

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
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

const CAL_PAGE: CalPage1 = CalPage1 {
    ampl: 100.0,
    period: 5.0,
    counter_max: 100,
};

//-----------------------------------------------------------------------------
// Demo task

// A task executed in multiple threads sharing a calibration parameter segment
fn demo_task(id: u32, cal_seg: CalSeg<CalPage1>) {
    // Create a thread local event instance
    // The capacity of the event capture buffer is 16 bytes
    let mut event = daq_create_event_tli!("demo_task", 16);
    println!("Task {id} started");

    // Demo signals
    let mut counter: u32 = 0;
    let mut sine: f64;

    loop {
        thread::sleep(Duration::from_millis(10)); // 100 Hz

        // A counter wrapping at a value specified by a calibration parameter
        counter += 1;
        if counter > cal_seg.counter_max {
            counter = 0
        }

        // A sine signal with amplitude and period from calibration parameters and an offset from thread id
        let time = START_TIME.elapsed().as_micros() as f64 * 0.000001; // s
        sine = (id as f64) * 10.0 + cal_seg.ampl * ((PI * time) / cal_seg.period).sin();

        // Register them once for each task instance and associate to the task instance event
        // Copy the value to the event capture buffer
        daq_capture_tli!(counter, event);
        daq_capture_tli!(sine, event, "sine: f64", "Volt", 1.0, 0.0);

        // Trigger the measurement event
        // Take a event timestamp send the captured data
        event.trigger();

        // Synchronize calibration operations
        // All calibration actions (download, page switch, freeze, init) on segment "calseg" happen here
        cal_seg.sync();
    }
}

//-----------------------------------------------------------------------------
// Demo application main

fn main() -> Result<()> {
    println!("XCPlite Multi Thread Demo");

    // Logging
    env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();

    // Initialize XCP
    let xcp = XcpBuilder::new("multi_thread_demo")
        .set_log_level(XcpLogLevel::Warn)
        .set_epk("EPK_12345678")
        .start_server(XcpTransportLayer::Udp, [127, 0, 0, 1], 5555)?;

    // Create a calibration parameter set (CalSeg in rust, MEMORY_SEGMENT in A2L) from a struct
    // Calibration segments have 2 pages, a constant default "FLASH" page and a mutable "RAM" page
    // FLASH or RAM can be switched at runtime (XCP set_cal_page), saved to json (XCP freeze) freeze and reinitialized from FLASH (XCP copy_cal_page)
    let calseg = xcp.create_calseg(
        "calseg",  // name of the calibration segment and the .json file
        &CAL_PAGE, // default calibration values
    );
    calseg.register_fields(); // Register all struct fields (with meta data from annotations) in the A2L registry

    // Start multiple instances of the demo task
    // Each instance will create its own measurement variable and event instances
    // The calibration segment is shared between the tasks (comparable to an Arc<Mutex>>)
    let mut t = Vec::new();
    for i in 0..=9 {
        t.push(thread::spawn({
            let calseg = CalSeg::clone(&calseg);
            move || {
                demo_task(i, calseg);
            }
        }));
    }

    // Test: Generate A2L immediately (normally this happens on XCP tool connect)
    // Wait some time until all threads have registered their measurement signals and events
    thread::sleep(Duration::from_millis(1000));
    xcp.write_a2l().unwrap();

    // Wait for the threads to finish
    t.into_iter().for_each(|t| t.join().unwrap());

    // Stop the XCP server
    xcp.stop_server();

    Ok(())
}
