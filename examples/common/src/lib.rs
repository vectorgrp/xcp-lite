//! Shared command line argument parser and logging setup for the xcp-lite examples.
//!
//! All examples accept the same options (transport, bind address, port, log level,
//! application name and `--flatten`). Factoring them out here keeps each example
//! focused on the actual measurement and calibration code instead of boilerplate.
//!
//! Usage in an example:
//! ```ignore
//! use example_common::ExampleArgs;
//!
//! let args = ExampleArgs::parse();
//! args.init_logging();
//! let app_name = args.app_name(APP_NAME);
//! // ... start the XCP server, then:
//! Xcp::get().set_registry_mode(args.flatten, false);
//! ```

use std::net::Ipv4Addr;

pub use clap::Parser;

const DEFAULT_LOG_LEVEL: u8 = 3; // Info
const DEFAULT_BIND_ADDR: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0); // ANY
const DEFAULT_PORT: u16 = 5555;
const DEFAULT_TCP: bool = false; // UDP

/// Command line arguments shared by all xcp-lite examples.
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct ExampleArgs {
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = DEFAULT_LOG_LEVEL)]
    pub log_level: u8,

    /// Bind address, default is ANY
    #[arg(short, long, default_value_t = DEFAULT_BIND_ADDR)]
    pub bind: Ipv4Addr,

    /// Use TCP as transport layer, default is UDP
    #[arg(short, long, default_value_t = DEFAULT_TCP)]
    pub tcp: bool,

    /// Port number
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Application name, defaults to the example name
    #[arg(short, long)]
    pub name: Option<String>,

    /// Flatten typedef structures into dot-mangled instance names in the A2L
    /// (for tools that do not support TYPEDEF_STRUCTURE). Default writes typedefs.
    #[arg(short, long, default_value_t = false)]
    pub flatten: bool,
}

impl ExampleArgs {
    /// Parse the command line arguments.
    pub fn parse() -> Self {
        <Self as Parser>::parse()
    }

    /// Application name from `--name`, or the example's default if not provided.
    pub fn app_name<'a>(&'a self, default: &'a str) -> &'a str {
        self.name.as_deref().unwrap_or(default)
    }

    /// Map the numeric log level to a `log::LevelFilter`.
    pub fn log_level_filter(&self) -> log::LevelFilter {
        match self.log_level {
            0 => log::LevelFilter::Off,
            1 => log::LevelFilter::Error,
            2 => log::LevelFilter::Warn,
            3 => log::LevelFilter::Info,
            4 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        }
    }

    /// Initialize `env_logger` with the selected log level (stdout, no timestamps).
    pub fn init_logging(&self) {
        env_logger::Builder::new()
            .target(env_logger::Target::Stdout)
            .filter_level(self.log_level_filter())
            .format_timestamp(None)
            .format_module_path(false)
            .format_target(false)
            .init();
    }
}
