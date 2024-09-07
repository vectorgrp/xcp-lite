// #![warn(rust_2018_idioms)]

use std::error::Error;
use std::io;
use std::net::SocketAddr;

//use once_cell::sync::OnceCell;
use tokio::net::UdpSocket;
use xcp::*;

struct Server {
    socket: UdpSocket,
    buf: Vec<u8>,
    xcp: &'static Xcp,
    client: Option<(usize, SocketAddr)>,
}

//static SERVER: OnceCell<Server> = OnceCell::new();

impl Server {
    async fn run(mut self) -> Result<(), io::Error> {
        loop {
            let client: (usize, SocketAddr) = self.socket.recv_from(&mut self.buf).await?;
            self.client = Some(client); // @@@@ check client changed
            self.xcp.tl_command(&self.buf);

            if let Some(buf) = self.xcp.tl_transmit_queue_peek() {
                self.socket.send_to(buf, &self.client.unwrap().1).await?;
                self.xcp.tl_transmit_queue_next();
            }
        }
    }
}

pub async fn start_server(addr: String) -> Result<&'static Xcp, Box<dyn Error>> {
    let socket = UdpSocket::bind(&addr).await?;
    println!("Bind to {}", socket.local_addr()?);

    // Initialize the XCP driver transport layer only, not the server
    let xcp = XcpBuilder::new("tokio_demo").set_log_level(XcpLogLevel::Debug).enable_a2l(true).start_protocol_layer().unwrap();

    // Start the tokio server
    let server = Server {
        socket,
        buf: vec![0; 1024],
        xcp,
        client: None,
    };

    // Run the server tasks
    server.run().await?;

    Ok(xcp)
}
