use xcp::*;
use xcp_type_description::prelude::*;

use libc::{SIGHUP, SIGINT, SIGTERM};
use log::info;
use serde::{Deserialize, Serialize};
use signal_hook::iterator::Signals;
use thiserror::Error;

use std::{sync::Arc, thread, time::Duration};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Max counter value", min = "0", max = "1023")]
    counter_max: u32,
    #[type_description(comment = "Min counter value", min = "0", max = "1023")]
    counter_min: u32,
    #[type_description(comment = "Task delay time in us", min = "0", max = "1000000", unit = "us")]
    delay: u32,
}

// Default value for the calibration parameters
const CAL_PAGE: CalPage1 = CalPage1 {
    counter_min: 5,
    counter_max: 10,
    delay: 100000,
};

#[derive(Error, Debug)]
enum XcpProcessError {
    #[error("An XCP error occurred: {0}")]
    XcpError(#[from] XcpError),
    #[error("XCP instance not initialized")]
    UninitializedError,
    #[error("General error: {0}")]
    GeneralError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

struct XcpProcess {
    xcp: Option<Arc<&'static Xcp>>,
    config: DaemonConfig,
}

impl XcpProcess {
    fn new(config: DaemonConfig) -> Self {
        XcpProcess { xcp: None, config: config }
    }

    fn get_xcp(&self) -> Result<&Xcp, XcpProcessError> {
        self.xcp.as_ref().map(|arc| **arc).ok_or(XcpProcessError::UninitializedError)
    }
}

impl Process for XcpProcess {
    type Error = XcpProcessError;

    fn init(&mut self) -> Result<(), Self::Error> {
        let xcp = XcpBuilder::new("xcp_daemon")
            .set_log_level(XcpLogLevel::Info)
            .set_epk("EPK_")
            .start_server(XcpTransportLayer::Udp, [172, 17, 247, 66], 5555)?;

        self.xcp = Some(Arc::new(xcp));

        Ok(())
    }

    fn run(&mut self) -> Result<(), Self::Error> {
        // Create a calibration segment with default values
        // and register the calibration parameters
        let xcp = self.get_xcp()?;
        let calseg = xcp.create_calseg("calseg", &CAL_PAGE);
        calseg.register_fields();

        // Measurement signal
        let mut counter: u32 = calseg.counter_min;
        let mut counter_u64: u64 = 0;

        // Register a measurement event and bind it to the measurement signal
        let mut event = daq_create_event!("mainloop", 16);

        let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGHUP]).map_err(|e| XcpProcessError::GeneralError(e.to_string()))?;

        let mut running = true;

        while running {
            for signal in signals.pending() {
                match signal {
                    SIGTERM => {
                        info!("Received SIGTERM signal");
                        running = false;
                        break;
                    }
                    SIGINT => {
                        info!("Received SIGINT signal");
                        running = false;
                        break;
                    }
                    SIGHUP => {
                        info!("Received SIGHUP signal");
                        for section in self.config.sections() {
                            for item in section.items {
                                info!("{} = {}", item.0, item.1);
                            }
                        }
                        break;
                    }
                    _ => {
                        info!("Received unknown signal");
                        break;
                    }
                }
            }

            if !running {
                break;
            }

            counter += 1;
            counter_u64 += 1;
            if counter > calseg.counter_max {
                counter = calseg.counter_min;
            }

            // Trigger timestamped measurement data acquisition of the counters
            daq_capture!(counter, event);
            daq_capture!(counter_u64, event);
            event.trigger();

            // Synchronize calibration parameters in calseg
            calseg.sync();

            xcp.write_a2l()?;

            thread::sleep(Duration::from_micros(calseg.delay as u64));
        }

        Ok(())
    }

    fn deinit(&mut self) -> Result<(), Self::Error> {
        info!("XCP shutting down.");
        let xcp = self.get_xcp()?;
        xcp.stop_server();
        std::fs::remove_file("xcp_daemon.a2l")?;
        Ok(())
    }
}

fn main() {
    let cfg_path = "/etc/xcp_demo_daemon/xcp_demo_daemon.conf";
    let config = DaemonConfig::new(cfg_path).unwrap();
    let mut daemon = Daemon::new(XcpProcess::new(config), "XcpProcess");
    match daemon.run() {
        Ok(_) => info!("Daemon run successful"),
        Err(e) => info!("Daemon run failed: {}", e),
    }
}
