use xcp::*;
use xcp_type_description::prelude::*;

use log::info;
use serde::{Deserialize, Serialize};
use signal_hook::{
    consts::{SIGHUP, SIGINT, SIGTERM},
    iterator::Signals,
};
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
    cfg: ProcessConfig,
}

impl XcpProcess {
    fn new(config: ProcessConfig) -> Self {
        XcpProcess { xcp: None, cfg: config }
    }

    fn get_xcp(&self) -> Result<&Xcp, XcpProcessError> {
        self.xcp.as_ref().map(|arc| **arc).ok_or(XcpProcessError::UninitializedError)
    }
}

impl Process for XcpProcess {
    type Error = XcpProcessError;

    fn init(&mut self) -> Result<(), Self::Error> {
        // env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(log::LevelFilter::Info).init();


        let host = self.config().sections().get_value("Server Config", "host").unwrap();
        let port = self.config().sections().get_value("Server Config", "port").unwrap();

        // Parse the host string into an array of integers
        let host: Vec<u8> = host.split('.').map(|s| s.parse().expect("Invalid IP address")).collect();
        let host: [u8; 4] = [host[0], host[1], host[2], host[3]];

        // Parse the port string into an integer
        let port: u16 = port.parse().expect("Invalid port number");

        let xcp = XcpBuilder::new(self.config().name())
            .set_log_level(XcpLogLevel::Info)
            .set_epk("EPK_")
            .start_server(XcpTransportLayer::Udp, host, port)?;

        self.xcp = Some(Arc::new(xcp));

        info!("XCP server initialized - {:?}:{}", host, port);

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
                        for section in self.config().sections().iterate() {
                            info!("Section: {}", section.name);
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
        std::fs::remove_file("xcpd.a2l")?;
        Ok(())
    }

    fn config(&self) -> &ProcessConfig {
        &self.cfg
    }
}

fn main() {
    let cfg = ProcessConfig::new(
        "xcpd",
        "/var/run/xcpd.pid",
        "/etc/xcpd/xcpd.conf",
        "/",
        "/var/log/xcpd.log",
        log::LevelFilter::Debug)
    .expect("Failed to create process config");

    let mut daemon = Daemon::new(XcpProcess::new(cfg));
    daemon.run().expect("Failed to run daemon");
}
