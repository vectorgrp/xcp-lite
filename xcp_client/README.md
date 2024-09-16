# xcp_client
XCP client implementation in Rust

Used for integration testing xcp-lite.  
Partial XCP implementation hard-coded for xcp-lite testing.  
Using tokio and a2lfile.  



  ``` rust

    // Create xcp_client
    let mut xcp_client = XcpClient::new("127.0.0.1:5555", "0.0.0.0:0");

    // Connect to the XCP server
    let res = xcp_client.connect(DaqDecoder::new(), ServTextDecoder::new()).await?;
    
    // Upload A2L file
    xcp_client.upload_a2l().await?;

    // Calibration
    // Create a calibration object for CalPage1.counter_max
    if let Ok(counter_max) = xcp_client.create_calibration_object("CalPage1.counter_max").await
    {
        // Get current value
        let v = xcp_client.get_value_u64(counter_max);
        info!("CalPage1.counter_max = {}", v);

        // Set value to 1000
        info!("CalPage1.counter_max = {}", v);
        xcp_client.set_value_u64(counter_max, 1000).await?;
    }

    // Measurement
    // Create a measurement for signal counter:u32
    xcp_client.create_measurement_object("counter").await?;
    xcp_client.start_measurement().await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    xcp_client.stop_measurement().await?;

    // Disconnect
    xcp_client.disconnect().await?);


   ```