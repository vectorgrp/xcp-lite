# xcp_client
XCP client implementation in Rust

Used for integration testing xcp-lite.  
Partial XCP implementation hard-coded for xcp-lite testing.  
Using tokio and a2lfile.  

xcp-lite-rdm % cargo r --example xcp_client -- -h

Usage: xcp_client [OPTIONS]

Options:
  -l, --log-level <LOG_LEVEL>
          Log level (Off=0, Error=1, Warn=2, Info=3, Debug=4, Trace=5) [default: 2]
  -d, --dest-addr <DEST_ADDR>
          XCP server address [default: 127.0.0.1:5555]
  -p, --port <PORT>
          XCP server port number [default: 5555]
  -b, --bind-addr <BIND_ADDR>
          Bind address, master port number [default: 0.0.0.0:9999]
      --print-a2l
          Print detailled A2L infos
      --list-mea
          Lists all measurement variables
      --list-cal
          Lists all calibration variables
  -m, --measurement-list <MEASUREMENT_LIST>...
          Specifies the variables names for DAQ measurement, 'all' or a list of names separated by space
  -a, --a2l-filename <A2L_FILENAME>
          A2L filename, default is upload A2L file
  -h, --help
          Print help
  -V, --version
          Print version



  ``` rust

    // Create xcp_client
    let mut xcp_client = XcpClient::new("127.0.0.1:5555", "0.0.0.0:0");

    // Connect to the XCP server
    let res = xcp_client.connect(DaqDecoder::new(), ServTextDecoder::new()).await?;
    
    // Upload A2L file or read A2L file
    xcp_client.upload_a2l(false).await?;
    xcp_client.read_a2l("test.a2l",false)?;

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
    xcp_client.init_measurement().await?;
    xcp_client.create_measurement_object("counter").await?;
    xcp_client.start_measurement().await?;
    sleep(Duration::from_secs(1)).await;
    xcp_client.stop_measurement().await?;

    // Disconnect
    xcp_client.disconnect().await?);


   ```