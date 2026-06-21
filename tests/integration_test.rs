use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use iroh_usbip::{
    UsbDeviceDescriptor, UsbConfigDescriptor, UsbSpeed,
    MockUsbDevice, engine::run_usbip_session,
};

#[tokio::test]
async fn test_op_req_devlist() -> anyhow::Result<()> {
    // 1. Setup mock device
    let desc = UsbDeviceDescriptor {
        vendor_id: 0x1234,
        product_id: 0x5678,
        device_class: 0x00,
        device_subclass: 0x00,
        device_protocol: 0x00,
        max_packet_size_0: 64,
        num_configurations: 1,
        usb_version: (2, 0),
        device_version: (1, 0),
        manufacturer_string_index: Some(1),
        product_string_index: Some(2),
        serial_number_string_index: Some(3),
    };
    let config = UsbConfigDescriptor {
        num_interfaces: 1,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![], // No interfaces for now to simplify
    };
    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: None,
        dropped: None,
    });

    // 2. Create in-memory duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(1024);

    // 3. Spawn host session handler
    let host_handle = tokio::spawn(async move {
        run_usbip_session(host_stream, vec![dev]).await
    });

    // 4. Send OP_REQ_DEVLIST from client
    let mut client = client_stream;
    let request_bytes: [u8; 8] = [
        0x01, 0x11, // version: 0x0111
        0x80, 0x05, // code: OP_REQ_DEVLIST (0x8005)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ];
    client.write_all(&request_bytes).await?;

    // 5. Read response from host
    let mut header = [0u8; 8];
    client.read_exact(&mut header).await?;
    
    // Check version
    assert_eq!(&header[0..2], &[0x01, 0x11]);
    // Check code (OP_REP_DEVLIST = 0x0005)
    assert_eq!(&header[2..4], &[0x00, 0x05]);
    // Check status (0)
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x00]);

    // Read ndev
    let mut ndev_buf = [0u8; 4];
    client.read_exact(&mut ndev_buf).await?;
    let ndev = u32::from_be_bytes(ndev_buf);
    assert_eq!(ndev, 1);

    // Read udev (312 bytes)
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // Verify vendor and product ID in serialized udev (offsets: idVendor is at 256 + 32 + 4 + 4 + 4 = 300)
    // Let's verify the layout offset for idVendor:
    // path: 256 bytes
    // busid: 32 bytes
    // busnum: 4 bytes
    // devnum: 4 bytes
    // speed: 4 bytes
    // idVendor: 2 bytes (offset 300..302)
    // idProduct: 2 bytes (offset 302..304)
    let vendor_id = u16::from_be_bytes([udev_buf[300], udev_buf[301]]);
    let product_id = u16::from_be_bytes([udev_buf[302], udev_buf[303]]);
    assert_eq!(vendor_id, 0x1234);
    assert_eq!(product_id, 0x5678);

    host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_op_req_import() -> anyhow::Result<()> {
    // 1. Setup mock device
    let desc = UsbDeviceDescriptor {
        vendor_id: 0x1234,
        product_id: 0x5678,
        device_class: 0x00,
        device_subclass: 0x00,
        device_protocol: 0x00,
        max_packet_size_0: 64,
        num_configurations: 1,
        usb_version: (2, 0),
        device_version: (1, 0),
        manufacturer_string_index: Some(1),
        product_string_index: Some(2),
        serial_number_string_index: Some(3),
    };
    let config = UsbConfigDescriptor {
        num_interfaces: 1,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![],
    };
    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: None,
        dropped: None,
    });

    // 2. Create in-memory duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(1024);

    // 3. Spawn host session handler
    let host_handle = tokio::spawn(async move {
        run_usbip_session(host_stream, vec![dev]).await
    });

    // 4. Send OP_REQ_IMPORT with correct busid "1-2"
    let mut client = client_stream;
    let mut req = Vec::new();
    req.extend_from_slice(&[
        0x01, 0x11, // version: 0x0111
        0x80, 0x03, // code: OP_REQ_IMPORT (0x8003)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ]);
    // busid (32 bytes)
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // 5. Read response from host
    let mut header = [0u8; 8];
    client.read_exact(&mut header).await?;
    
    // Check version
    assert_eq!(&header[0..2], &[0x01, 0x11]);
    // Check code (OP_REP_IMPORT = 0x0003)
    assert_eq!(&header[2..4], &[0x00, 0x03]);
    // Check status (0)
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x00]);

    // Read udev (312 bytes)
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    let vendor_id = u16::from_be_bytes([udev_buf[300], udev_buf[301]]);
    let product_id = u16::from_be_bytes([udev_buf[302], udev_buf[303]]);
    assert_eq!(vendor_id, 0x1234);
    assert_eq!(product_id, 0x5678);

    drop(client);
    host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_urb_transfer() -> anyhow::Result<()> {
    // 1. Setup mock device with a transfer handler callback
    let desc = UsbDeviceDescriptor {
        vendor_id: 0x1234,
        product_id: 0x5678,
        device_class: 0x00,
        device_subclass: 0x00,
        device_protocol: 0x00,
        max_packet_size_0: 64,
        num_configurations: 1,
        usb_version: (2, 0),
        device_version: (1, 0),
        manufacturer_string_index: Some(1),
        product_string_index: Some(2),
        serial_number_string_index: Some(3),
    };
    let config = UsbConfigDescriptor {
        num_interfaces: 1,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![],
    };

    // Define the custom callback to return mock descriptor data on control read
    let callback = Arc::new(|action: String, _data: Vec<u8>| {
        if action == "control_read:128:6:256:0" {
            // request_type=128 (0x80), request=6, value=256 (0x0100), index=0
            Ok(vec![0x12, 0x34, 0x56, 0x78])
        } else {
            Ok(vec![])
        }
    });

    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: Some(callback),
        dropped: None,
    });

    // 2. Create duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(1024);

    // 3. Spawn host session handler
    let host_handle = tokio::spawn(async move {
        run_usbip_session(host_stream, vec![dev]).await
    });

    // 4. Send OP_REQ_IMPORT
    let mut client = client_stream;
    let mut req = Vec::new();
    req.extend_from_slice(&[
        0x01, 0x11, // version: 0x0111
        0x80, 0x03, // code: OP_REQ_IMPORT (0x8003)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ]);
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // Read import response
    let mut import_header = [0u8; 8];
    client.read_exact(&mut import_header).await?;
    assert_eq!(&import_header[4..8], &[0, 0, 0, 0]); // success
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // 5. Send USBIP_CMD_SUBMIT (Control Read, ep 0, 48-byte header)
    let mut cmd = Vec::new();
    // Basic header:
    cmd.extend_from_slice(&0x0001u32.to_be_bytes()); // command: USBIP_CMD_SUBMIT (1)
    cmd.extend_from_slice(&10u32.to_be_bytes());      // seqnum: 10
    cmd.extend_from_slice(&0x00010002u32.to_be_bytes()); // devid: (1<<16)|2
    cmd.extend_from_slice(&1u32.to_be_bytes());      // direction: IN (1)
    cmd.extend_from_slice(&0u32.to_be_bytes());      // ep: 0

    // Submit header fields:
    cmd.extend_from_slice(&0u32.to_be_bytes());      // transfer_flags
    cmd.extend_from_slice(&4i32.to_be_bytes());      // transfer_buffer_length: 4
    cmd.extend_from_slice(&0i32.to_be_bytes());      // start_frame
    cmd.extend_from_slice(&0i32.to_be_bytes());      // number_of_packets
    cmd.extend_from_slice(&0i32.to_be_bytes());      // interval
    cmd.extend_from_slice(&[0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 0x04, 0x00]); // setup (8 bytes)

    client.write_all(&cmd).await?;

    // 6. Read response (48-byte header + 4-byte payload)
    let mut ret_header = [0u8; 48];
    client.read_exact(&mut ret_header).await?;

    // Verify RET_SUBMIT header
    let ret_command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
    let ret_seqnum = u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
    let ret_status = i32::from_be_bytes([ret_header[20], ret_header[21], ret_header[22], ret_header[23]]);
    let ret_actual_len = i32::from_be_bytes([ret_header[24], ret_header[25], ret_header[26], ret_header[27]]);

    assert_eq!(ret_command, 0x0003); // USBIP_RET_SUBMIT (3)
    assert_eq!(ret_seqnum, 10);
    assert_eq!(ret_status, 0);
    assert_eq!(ret_actual_len, 4);

    // Read payload
    let mut payload = [0u8; 4];
    client.read_exact(&mut payload).await?;
    assert_eq!(&payload, &[0x12, 0x34, 0x56, 0x78]);

    // Drop connection to let host finish
    drop(client);
    let _ = host_handle.await;
    Ok(())
}

#[tokio::test]
async fn test_urb_unlink() -> anyhow::Result<()> {
    // 1. Setup mock device
    let desc = UsbDeviceDescriptor {
        vendor_id: 0x1234,
        product_id: 0x5678,
        device_class: 0x00,
        device_subclass: 0x00,
        device_protocol: 0x00,
        max_packet_size_0: 64,
        num_configurations: 1,
        usb_version: (2, 0),
        device_version: (1, 0),
        manufacturer_string_index: Some(1),
        product_string_index: Some(2),
        serial_number_string_index: Some(3),
    };
    let config = UsbConfigDescriptor {
        num_interfaces: 1,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![],
    };
    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: None,
        dropped: None,
    });

    // 2. Create duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(1024);

    // 3. Spawn host session handler
    let host_handle = tokio::spawn(async move {
        run_usbip_session(host_stream, vec![dev]).await
    });

    // 4. Send OP_REQ_IMPORT
    let mut client = client_stream;
    let mut req = Vec::new();
    req.extend_from_slice(&[
        0x01, 0x11, // version: 0x0111
        0x80, 0x03, // code: OP_REQ_IMPORT (0x8003)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ]);
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // Read import response
    let mut import_header = [0u8; 8];
    client.read_exact(&mut import_header).await?;
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // 5. Send USBIP_CMD_UNLINK (48-byte header)
    let mut cmd = Vec::new();
    cmd.extend_from_slice(&0x0002u32.to_be_bytes()); // command: USBIP_CMD_UNLINK (2)
    cmd.extend_from_slice(&20u32.to_be_bytes());      // seqnum: 20
    cmd.extend_from_slice(&0x00010002u32.to_be_bytes()); // devid: (1<<16)|2
    cmd.extend_from_slice(&0u32.to_be_bytes());      // direction: OUT (0)
    cmd.extend_from_slice(&0u32.to_be_bytes());      // ep: 0

    // Unlink header fields:
    cmd.extend_from_slice(&10u32.to_be_bytes());      // seqnum of original command to unlink: 10
    cmd.extend_from_slice(&[0; 24]);                  // padding: 24 bytes of zeroes

    client.write_all(&cmd).await?;

    // 6. Read response (48-byte header)
    let mut ret_header = [0u8; 48];
    client.read_exact(&mut ret_header).await?;

    // Verify RET_UNLINK header
    let ret_command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
    let ret_seqnum = u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
    let ret_status = i32::from_be_bytes([ret_header[20], ret_header[21], ret_header[22], ret_header[23]]);

    assert_eq!(ret_command, 0x0004); // USBIP_RET_UNLINK (4)
    assert_eq!(ret_seqnum, 20);
    assert_eq!(ret_status, -104);    // -ECONNRESET

    // Drop connection to let host finish
    drop(client);
    let _ = host_handle.await;
    Ok(())
}

#[tokio::test]
async fn test_disconnection_teardown() -> anyhow::Result<()> {
    // 1. Setup mock device with a dropped flag
    let desc = UsbDeviceDescriptor {
        vendor_id: 0x1234,
        product_id: 0x5678,
        device_class: 0x00,
        device_subclass: 0x00,
        device_protocol: 0x00,
        max_packet_size_0: 64,
        num_configurations: 1,
        usb_version: (2, 0),
        device_version: (1, 0),
        manufacturer_string_index: Some(1),
        product_string_index: Some(2),
        serial_number_string_index: Some(3),
    };
    let config = UsbConfigDescriptor {
        num_interfaces: 1,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![],
    };
    let dropped_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: None,
        dropped: Some(dropped_flag.clone()),
    });

    // 2. Create duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(1024);

    // 3. Spawn host session handler
    let host_handle = tokio::spawn(async move {
        run_usbip_session(host_stream, vec![dev]).await
    });

    // 4. Send OP_REQ_IMPORT
    let mut client = client_stream;
    let mut req = Vec::new();
    req.extend_from_slice(&[
        0x01, 0x11, // version: 0x0111
        0x80, 0x03, // code: OP_REQ_IMPORT (0x8003)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ]);
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // Read import response
    let mut import_header = [0u8; 8];
    client.read_exact(&mut import_header).await?;
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // 5. Drop client connection
    drop(client);

    // 6. Wait for host session to terminate
    host_handle.await??;

    // 7. Verify that device handle was dropped
    assert!(dropped_flag.load(std::sync::atomic::Ordering::SeqCst));

    Ok(())
}


