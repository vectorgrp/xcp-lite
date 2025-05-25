// lib_demo
// xcp-lite xcplib c-api demo
//
// Demonstrates the usage of xcp-lite xcplib API
//
// Run the demo
// cargo run --example lib_demo
//
// Run the test XCP client in another terminal or start CANape with the project in folder examples/hello_xcp/CANape
// cargo run --example xcp_client -- -m "counter"

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use clap::Parser;

//-----------------------------------------------------------------------------
// Command line arguments

const DEFAULT_LOG_LEVEL: u8 = 3; // Info
const DEFAULT_BIND_ADDR: std::net::Ipv4Addr = std::net::Ipv4Addr::new(0, 0, 0, 0); // ANY
const DEFAULT_PORT: u16 = 5555;
const DEFAULT_TCP: bool = false; // UDP

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5)
    #[arg(short, long, default_value_t = DEFAULT_LOG_LEVEL)]
    log_level: u8,

    /// Bind address, default is ANY
    #[arg(short, long, default_value_t = DEFAULT_BIND_ADDR)]
    bind: std::net::Ipv4Addr,

    /// Use TCP as transport layer, default is UDP
    #[arg(short, long, default_value_t = DEFAULT_TCP)]
    tcp: bool,

    /// Port number
    #[arg(short, long, default_value_t = DEFAULT_PORT)]
    port: u16,
}

//-----------------------------------------------------------------------------

fn main() {
    println!("xcplib demo  - CANape project in ./examples/lib_demo/CANape");

    // Args
    let _args = Args::parse();

    unsafe {
        xcp_lite::ApplXcpSetLogLevel(_args.log_level);
    }

    unsafe { xcp_lite::c_demo() }
}
