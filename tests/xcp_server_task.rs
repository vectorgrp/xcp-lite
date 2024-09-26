pub async fn xcp_task<A>(xcp: &'static xcp::Xcp, addr: A, port: u16) -> Result<(), std::io::Error>
where
    A: Into<std::net::Ipv4Addr>,
{
    log::info!("xcp_task: start");

    // Bind to address
    let addr = addr.into();
    let socket = tokio::net::UdpSocket::bind((addr, port)).await?;
    log::info!("xcp_task: bind to {}:{}", addr, port);

    let mut client_addr = None;
    let mut buf = vec![0u8; 8000];

    loop {
        let rx_future = socket.recv_from(&mut buf);
        let res = tokio::time::timeout(tokio::time::Duration::from_millis(10), rx_future).await;
        match res {
            Err(_) => {
                log::trace!("xcp_task: timeout");
            }

            Ok(rx) => match rx {
                Err(e) => {
                    log::error!("xcp_task: xcp_task stop, recv error: {}", e);
                    return Err(e);
                }

                Ok((size, addr)) => {
                    if size == 0 {
                        log::warn!("xcp_task: xcp_task stop, recv 0 bytes from {}, socket closed", addr);
                        return Ok(());
                    } else {
                        log::trace!("xcp_task: recv {} bytes from {}, buf_len={}", size, addr, buf.len());

                        // Set client address, do not accept new clients while being connected
                        if let Some(c) = client_addr {
                            if c != addr && xcp.is_connected() {
                                log::error!("xcp_task: client addr changed to {} while beeing connected to {}", addr, c);
                                assert_eq!(c, addr);
                            }
                        } else {
                            client_addr = Some(addr);
                            log::info!("xcp_task: set client to {}", addr);
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
            log::trace!("xcp_task: read transmit queue ");

            if xcp.tl_transmit_queue_has_msg() {
                // Empty the transmit queue
                while let Some(buf) = xcp.tl_transmit_queue_peek() {
                    socket.send_to(buf, addr).await?;
                    xcp.tl_transmit_queue_next();
                    log::trace!("xcp_task: Sent {} bytes to {}", buf.len(), client_addr.unwrap());
                }
            }
        }
    } // loop
}
