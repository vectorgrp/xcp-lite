use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use log::{error, info, trace, warn};

use tokio::net::UdpSocket;
use tokio::time::timeout;

use xcp::*;

pub async fn xcp_task<A>(xcp: &'static Xcp, addr: A, port: u16) -> Result<(), io::Error>
where
    A: Into<Ipv4Addr>,
{
    info!("xcp_task: start");

    // Bind to address
    let addr = addr.into();
    let socket = UdpSocket::bind((addr, port)).await?;
    info!("xcp_task: bind to {}:{}", addr, port);

    let mut client_addr = None;
    let mut buf = vec![0u8; 1024];

    loop {
        let rx_future = socket.recv_from(&mut buf);
        let res = timeout(Duration::from_millis(10), rx_future).await;
        match res {
            Err(_) => {
                trace!("xcp_task: timeout");
            }

            Ok(rx) => match rx {
                Err(e) => {
                    error!("xcp_task: xcp_task stop, recv error: {}", e);
                    return Err(e);
                }

                Ok((size, addr)) => {
                    if size == 0 {
                        warn!("xcp_task: xcp_task stop, recv 0 bytes from {}, socket closed", addr);
                        return Ok(());
                    } else {
                        info!("xcp_task: recv {} bytes from {}, buf_len={}", size, addr, buf.len());

                        // Set client address, do not accept new clients while being connected
                        if let Some(c) = client_addr {
                            if c != addr && xcp.is_connected() {
                                error!("xcp_task: client addr changed to {} while beeing connected to {}", addr, c);
                                assert_eq!(c, addr);
                            }
                        } else {
                            client_addr = Some(addr);
                            info!("xcp_task: set client to {}", addr);
                        }

                        // Execute command
                        xcp.tl_command(&buf);
                    }
                }
            }, // match
        } // match res

        // Transmit
        // Check if client address is valid
        if let Some(addr) = client_addr {
            trace!("xcp_task: read transmit queue ");

            // Empty the transmit queue
            while let Some(buf) = xcp.tl_transmit_queue_peek() {
                socket.send_to(buf, addr).await?;
                xcp.tl_transmit_queue_next();
                info!("xcp_task: Sent {} bytes to {}", buf.len(), client_addr.unwrap());
            }
        }
    } // loop
}