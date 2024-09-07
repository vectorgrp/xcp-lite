// #![warn(rust_2018_idioms)]

use std::error::Error;
use std::io;
use std::net::SocketAddr;

use log::info;

use once_cell::sync::OnceCell;

//use tokio::join;
use tokio::net::UdpSocket;

use xcp::*;

#[derive(Debug)]
struct Server {
    socket: UdpSocket,
}

#[derive(Debug)]
struct Client {
    client: SocketAddr,
}

static ASYNC_XCP_SERVER: OnceCell<Server> = OnceCell::new();
static ASYNC_XCP_CLIENT: OnceCell<Client> = OnceCell::new();

async fn rx_task() -> Result<(), io::Error> {
    let server = ASYNC_XCP_SERVER.get().unwrap();
    let xcp = Xcp::get();
    let mut buf = vec![0u8; 1024];
    loop {
        let res: (usize, SocketAddr) = server.socket.recv_from(&mut buf).await?;
        info!("rx_task: recv {} bytes from {}, buf_len={}", res.0, res.1, buf.len());

        if let Some(c) = ASYNC_XCP_CLIENT.get() {
            assert_eq!(c.client, res.1);
        } else {
            ASYNC_XCP_CLIENT.set(Client { client: res.1 }).unwrap();
        }

        xcp.tl_command(&buf);
    }
}

async fn tx_task() -> Result<(), io::Error> {
    let server = ASYNC_XCP_SERVER.get().unwrap();

    let xcp = Xcp::get();
    loop {
        while let Some(buf) = xcp.tl_transmit_queue_peek() {
            let client = ASYNC_XCP_CLIENT.get().unwrap();
            server.socket.send_to(buf, &client.client).await?;
            xcp.tl_transmit_queue_next();
            info!("Sent {} bytes to {}", buf.len(), client.client);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
    }
}

pub async fn start_async_xcp_server(addr: String) -> Result<(tokio::task::JoinHandle<Result<(), io::Error>>, tokio::task::JoinHandle<Result<(), io::Error>>), Box<dyn Error>> {
    let socket = UdpSocket::bind(&addr).await?;
    println!("Bind to {}", socket.local_addr()?);

    // Initialize the XCP driver transport layer only, not the server
    let _xcp = XcpBuilder::new("tokio_demo").set_log_level(XcpLogLevel::Debug).enable_a2l(true).start_protocol_layer().unwrap();

    // Start the tokio server
    let server = Server { socket };
    if ASYNC_XCP_SERVER.get().is_some() {
        return Err("Server already started".into());
    }
    ASYNC_XCP_SERVER.set(server).unwrap();
    let rx_task = tokio::spawn(rx_task());
    let tx_task: tokio::task::JoinHandle<Result<(), io::Error>> = tokio::spawn(tx_task());

    Ok((rx_task, tx_task))
}
