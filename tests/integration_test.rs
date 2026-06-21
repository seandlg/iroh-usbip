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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
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

#[tokio::test]
async fn test_op_req_import_device_busy() -> anyhow::Result<()> {
    // 1. Setup mock device that fails open with "device is busy"
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
        open_error: Some("device is busy".to_string()),
        kernel_drivers: None,
        claimed_interfaces: None,
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
    // Check status (ST_DEV_BUSY = 0x02)
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x02]);

    drop(client);
    host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_op_req_import_device_not_available() -> anyhow::Result<()> {
    // 1. Setup mock device that fails open with generic error
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
        open_error: Some("general permission/hardware error".to_string()),
        kernel_drivers: None,
        claimed_interfaces: None,
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
    // Check status (ST_NA = 0x01)
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x01]);

    drop(client);
    host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_isochronous_transfer_dummy_handling() -> anyhow::Result<()> {
    use iroh_usbip::protocol::UsbipIsoPacketDescriptor;

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

    let callback = Arc::new(|action: String, _data: Vec<u8>| {
        if action == "bulk_read:129" {
            Ok(vec![0xAA; 64])
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
        open_error: None,
        kernel_drivers: None,
        claimed_interfaces: None,
    });

    // 2. Create duplex stream
    let (client_stream, host_stream) = tokio::io::duplex(2048);

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
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // Read import response
    let mut import_header = [0u8; 8];
    client.read_exact(&mut import_header).await?;
    assert_eq!(&import_header[4..8], &[0, 0, 0, 0]); // success
    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // 5. Send USBIP_CMD_SUBMIT (Isochronous IN, ep 1, 48-byte header + 2 descriptors)
    let mut cmd = Vec::new();
    // Basic header:
    cmd.extend_from_slice(&0x0001u32.to_be_bytes()); // command: USBIP_CMD_SUBMIT (1)
    cmd.extend_from_slice(&10u32.to_be_bytes());      // seqnum: 10
    cmd.extend_from_slice(&0x00010002u32.to_be_bytes()); // devid: (1<<16)|2
    cmd.extend_from_slice(&1u32.to_be_bytes());      // direction: IN (1)
    cmd.extend_from_slice(&1u32.to_be_bytes());      // ep: 1

    // Submit header fields:
    cmd.extend_from_slice(&0u32.to_be_bytes());      // transfer_flags
    cmd.extend_from_slice(&64i32.to_be_bytes());     // transfer_buffer_length: 64
    cmd.extend_from_slice(&0i32.to_be_bytes());      // start_frame
    cmd.extend_from_slice(&2i32.to_be_bytes());      // number_of_packets: 2
    cmd.extend_from_slice(&0i32.to_be_bytes());      // interval
    cmd.extend_from_slice(&[0; 8]);                  // setup (8 bytes)

    // Send 2 packet descriptors
    let desc1 = UsbipIsoPacketDescriptor {
        offset: 0,
        length: 32,
        actual_length: 0,
        status: 0,
    };
    let desc2 = UsbipIsoPacketDescriptor {
        offset: 32,
        length: 32,
        actual_length: 0,
        status: 0,
    };
    cmd.extend_from_slice(&desc1.to_bytes());
    cmd.extend_from_slice(&desc2.to_bytes());

    client.write_all(&cmd).await?;

    // 6. Read response (48-byte header + 64-byte payload + 32-byte dummy descriptors)
    let mut ret_header = [0u8; 48];
    client.read_exact(&mut ret_header).await?;

    // Verify RET_SUBMIT header
    let ret_command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
    let ret_seqnum = u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
    let ret_status = i32::from_be_bytes([ret_header[20], ret_header[21], ret_header[22], ret_header[23]]);
    let ret_actual_len = i32::from_be_bytes([ret_header[24], ret_header[25], ret_header[26], ret_header[27]]);
    let ret_packets = i32::from_be_bytes([ret_header[32], ret_header[33], ret_header[34], ret_header[35]]);

    assert_eq!(ret_command, 0x0003); // USBIP_RET_SUBMIT (3)
    assert_eq!(ret_seqnum, 10);
    assert_eq!(ret_status, 0);
    assert_eq!(ret_actual_len, 64);
    assert_eq!(ret_packets, 2);

    // Read payload
    let mut payload = vec![0u8; 64];
    client.read_exact(&mut payload).await?;
    assert_eq!(payload, vec![0xAA; 64]);

    // Read dummy descriptors (32 bytes)
    let mut returned_descs = [0u8; 32];
    client.read_exact(&mut returned_descs).await?;

    let mut rdesc1_bytes = [0u8; 16];
    let mut rdesc2_bytes = [0u8; 16];
    rdesc1_bytes.copy_from_slice(&returned_descs[0..16]);
    rdesc2_bytes.copy_from_slice(&returned_descs[16..32]);

    let rdesc1 = UsbipIsoPacketDescriptor::from_bytes(rdesc1_bytes);
    let rdesc2 = UsbipIsoPacketDescriptor::from_bytes(rdesc2_bytes);

    assert_eq!(rdesc1.offset, 0);
    assert_eq!(rdesc1.length, 32);
    assert_eq!(rdesc1.actual_length, 32);
    assert_eq!(rdesc1.status, 0);

    assert_eq!(rdesc2.offset, 32);
    assert_eq!(rdesc2.length, 32);
    assert_eq!(rdesc2.actual_length, 32);
    assert_eq!(rdesc2.status, 0);

    // 7. Verify stream synchronization by sending a subsequent unlink command
    let mut unlink_cmd = Vec::new();
    unlink_cmd.extend_from_slice(&0x0002u32.to_be_bytes()); // command: USBIP_CMD_UNLINK (2)
    unlink_cmd.extend_from_slice(&20u32.to_be_bytes());      // seqnum: 20
    unlink_cmd.extend_from_slice(&0x00010002u32.to_be_bytes()); // devid: (1<<16)|2
    unlink_cmd.extend_from_slice(&0u32.to_be_bytes());      // direction: OUT (0)
    unlink_cmd.extend_from_slice(&0u32.to_be_bytes());      // ep: 0
    unlink_cmd.extend_from_slice(&10u32.to_be_bytes());      // seqnum of original command to unlink: 10
    unlink_cmd.extend_from_slice(&[0; 24]);                  // padding: 24 bytes of zeroes

    client.write_all(&unlink_cmd).await?;

    // Read unlink response
    let mut unlink_ret_header = [0u8; 48];
    client.read_exact(&mut unlink_ret_header).await?;
    let unlink_ret_command = u32::from_be_bytes([unlink_ret_header[0], unlink_ret_header[1], unlink_ret_header[2], unlink_ret_header[3]]);
    let unlink_ret_seqnum = u32::from_be_bytes([unlink_ret_header[4], unlink_ret_header[5], unlink_ret_header[6], unlink_ret_header[7]]);
    assert_eq!(unlink_ret_command, 0x0004); // USBIP_RET_UNLINK (4)
    assert_eq!(unlink_ret_seqnum, 20);

    // Drop connection to let host finish
    drop(client);
    let _ = host_handle.await;
    Ok(())
}

#[tokio::test]
async fn test_driver_detachment_lifecycle() -> anyhow::Result<()> {
    // 1. Setup shared state to monitor mock device handle interactions
    let kernel_drivers = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let claimed_interfaces = Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));

    // Suppose our device configuration has interface 0 and interface 1
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
        manufacturer_string_index: None,
        product_string_index: None,
        serial_number_string_index: None,
    };

    use iroh_usbip::{UsbInterfaceDescriptor, UsbInterfaceSettingDescriptor};
    let config = UsbConfigDescriptor {
        num_interfaces: 2,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![
            UsbInterfaceDescriptor {
                interface_number: 0,
                settings: vec![UsbInterfaceSettingDescriptor {
                    setting_number: 0,
                    class_code: 0,
                    sub_class_code: 0,
                    protocol_code: 0,
                    endpoints: vec![],
                }],
            },
            UsbInterfaceDescriptor {
                interface_number: 1,
                settings: vec![UsbInterfaceSettingDescriptor {
                    setting_number: 0,
                    class_code: 0,
                    sub_class_code: 0,
                    protocol_code: 0,
                    endpoints: vec![],
                }],
            },
        ],
    };

    // Pre-populate active kernel drivers for both interfaces
    kernel_drivers.lock().unwrap().insert(0, true);
    kernel_drivers.lock().unwrap().insert(1, true);

    let dev = Arc::new(MockUsbDevice {
        bus_num: 1,
        dev_addr: 2,
        dev_speed: UsbSpeed::High,
        descriptor: desc,
        config_descriptor: config,
        transfer_handler: None,
        dropped: None,
        open_error: None,
        kernel_drivers: Some(kernel_drivers.clone()),
        claimed_interfaces: Some(claimed_interfaces.clone()),
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
    let busid_bytes = iroh_usbip::protocol::pad_string("1-2", 32);
    req.extend_from_slice(&busid_bytes);
    client.write_all(&req).await?;

    // 5. Read response from host (header + udev)
    let mut header = [0u8; 8];
    client.read_exact(&mut header).await?;
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x00]); // success

    let mut udev_buf = [0u8; 312];
    client.read_exact(&mut udev_buf).await?;

    // 6. Assert that during client connection, kernel drivers are detached, and interfaces are claimed
    {
        let kd = kernel_drivers.lock().unwrap();
        assert_eq!(kd.get(&0), Some(&false)); // Interface 0 detached
        assert_eq!(kd.get(&1), Some(&false)); // Interface 1 detached

        let ci = claimed_interfaces.lock().unwrap();
        assert!(ci.contains(&0)); // Interface 0 claimed
        assert!(ci.contains(&1)); // Interface 1 claimed
    }

    // 7. Disconnect the client
    drop(client);
    host_handle.await??;

    // 8. Assert that after host teardown, interfaces are released and kernel drivers are re-attached
    {
        let kd = kernel_drivers.lock().unwrap();
        assert_eq!(kd.get(&0), Some(&true)); // Interface 0 re-attached
        assert_eq!(kd.get(&1), Some(&true)); // Interface 1 re-attached

        let ci = claimed_interfaces.lock().unwrap();
        assert!(!ci.contains(&0)); // Interface 0 released
        assert!(!ci.contains(&1)); // Interface 1 released
    }

    Ok(())
}



