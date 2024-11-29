#[cfg(unix)]
mod platform;
#[cfg(unix)]
use platform::*;
#[cfg(unix)]
use log::info;

use log::error;

#[cfg(unix)]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy, XcpTypeDescription)]
struct CalPage1 {
    #[type_description(comment = "Max counter value", min = "0", max = "1023")]
    counter_max: u32,
    #[type_description(comment = "Min counter value", min = "0", max = "1023")]
    counter_min: u32,
    #[type_description(comment = "Task delay time in us", min = "0", max = "1000000", unit = "us")]
    delay: u32,
}

#[cfg(unix)]
// Default value for the calibration parameters
const CAL_PAGE: CalPage1 = CalPage1 {
    counter_min: 5,
    counter_max: 10,
    delay: 100000,
};

#[cfg(unix)]
#[derive(Error, Debug)]
enum XcpProcessError {
    #[error("An XCP error occurred: {0}")]
    XcpError(#[from] XcpError),
    #[error("General error: {0}")]
    GeneralError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg(unix)]
struct XcpProcess {
    cfg: ProcessConfig,
}

#[cfg(unix)]
impl XcpProcess {
    fn new(config: ProcessConfig) -> Self {
        XcpProcess { cfg: config }
    }
}

#[cfg(unix)]
impl Process for XcpProcess {
    type Error = XcpProcessError;

    fn init(&mut self) -> Result<(), Self::Error> {
        // Defaults
        let mut server_addr: std::net::Ipv4Addr = [0, 0, 0, 0].into();
        let mut server_port: u16 = 5555;
        let mut server_log_level: u8 = 2; // Warn

        // Read from config
        if let Some(s) = self.config().sections().get_value("Server Config", "host") {
            if let Ok(h) = s.parse() {
                server_addr = h;
            }
        }
        if let Some(s) = self.config().sections().get_value("Server Config", "port") {
            if let Ok(p) = s.parse() {
                server_port = p;
            }
        }
        if let Some(l) = self.config().sections().get_value("Server Config", "log_level") {
            server_log_level = l.parse().unwrap_or(2);
        }
        let daemon_log_level = match server_log_level {
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            5 => log::LevelFilter::Trace,
            _ => log::LevelFilter::Error,
        };

        // Logger
        env_logger::Builder::new().target(env_logger::Target::Stdout).filter_level(daemon_log_level).init();

        // XCP
        XcpBuilder::new(self.config().name())
            .set_log_level(server_log_level)
            .set_epk("EPK_")
            .start_server(XcpTransportLayer::Udp, server_addr, server_port)?;

        info!("XCP server initialized - {:?}:{}", server_addr, server_port);

        Ok(())
    }

    fn run(&mut self) -> Result<(), Self::Error> {
        // Create a calibration segment with default values
        // and register the calibration parameters
        let xcp = Xcp::get();
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
        let xcp = Xcp::get();
        xcp.stop_server();
        std::fs::remove_file(format!("{}.a2l", self.config().name()))?;
        Ok(())
    }

    fn config(&self) -> &ProcessConfig {
        &self.cfg
    }
}
#[cfg(unix)]
fn _main() {
    let cfg = ProcessConfig::new(
        "xcpd",
        "/var/run/xcpd.pid",
        "/etc/xcpd/xcpd.conf",
        "/",
        "/var/log/xcpd.log",
        "/var/log/xcpd.log",
        "/var/log/xcpd.log",
    )
    .expect("Failed to create process config");

    let mut daemon = Daemon::new(XcpProcess::new(cfg));
    daemon.run().expect("Failed to run daemon");
}

#[cfg(not(unix))]
fn _main() {
    error!("Daemonization is only supported for Unix platforms.");
}

fn main() {
    _main();
}
