#![allow(dead_code)]

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::task::JoinHandle;
use iroh_usbip::{
    UsbDeviceDescriptor, UsbConfigDescriptor, UsbSpeed,
    MockUsbDevice, MockTransferCallback, engine::run_usbip_session,
};
use iroh_usbip::protocol::UsbipIsoPacketDescriptor;

pub struct TestContext {
    pub client: DuplexStream,
    pub host_handle: JoinHandle<anyhow::Result<()>>,
}

impl TestContext {
    pub async fn new(device: Arc<MockUsbDevice>) -> Self {
        Self::new_multi(vec![device]).await
    }

    pub async fn new_multi(devices: Vec<Arc<MockUsbDevice>>) -> Self {
        let (client_stream, host_stream) = tokio::io::duplex(2048);
        let host_handle = tokio::spawn(async move {
            run_usbip_session(host_stream, devices).await
        });
        Self {
            client: client_stream,
            host_handle,
        }
    }

    // Send OP_REQ_DEVLIST
    pub async fn send_op_req_devlist(&mut self) -> anyhow::Result<()> {
        let request_bytes: [u8; 8] = [
            0x01, 0x11, // version: 0x0111
            0x80, 0x05, // code: OP_REQ_DEVLIST (0x8005)
            0x00, 0x00, 0x00, 0x00, // status: 0
        ];
        self.client.write_all(&request_bytes).await?;
        Ok(())
    }

    // Read OP_REP_DEVLIST
    // Returns (ndev, list of udev bytes)
    pub async fn read_op_rep_devlist(&mut self) -> anyhow::Result<(u32, Vec<Vec<u8>>)> {
        let mut header = [0u8; 8];
        self.client.read_exact(&mut header).await?;
        assert_eq!(&header[0..2], &[0x01, 0x11]);
        assert_eq!(&header[2..4], &[0x00, 0x05]); // OP_REP_DEVLIST
        assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x00]);

        let mut ndev_buf = [0u8; 4];
        self.client.read_exact(&mut ndev_buf).await?;
        let ndev = u32::from_be_bytes(ndev_buf);

        let mut devices = Vec::new();
        for _ in 0..ndev {
            let mut udev_buf = [0u8; 312];
            self.client.read_exact(&mut udev_buf).await?;
            devices.push(udev_buf.to_vec());
        }
        Ok((ndev, devices))
    }

    // Send OP_REQ_IMPORT
    pub async fn send_op_req_import(&mut self, busid: &str) -> anyhow::Result<()> {
        let mut req = Vec::new();
        req.extend_from_slice(&[
            0x01, 0x11, // version: 0x0111
            0x80, 0x03, // code: OP_REQ_IMPORT (0x8003)
            0x00, 0x00, 0x00, 0x00, // status: 0
        ]);
        let busid_bytes = iroh_usbip::protocol::pad_string(busid, 32);
        req.extend_from_slice(&busid_bytes);
        self.client.write_all(&req).await?;
        Ok(())
    }

    // Read OP_REP_IMPORT
    // Returns (status, udev_buf)
    pub async fn read_op_rep_import(&mut self) -> anyhow::Result<(u32, Vec<u8>)> {
        let mut header = [0u8; 8];
        self.client.read_exact(&mut header).await?;
        assert_eq!(&header[0..2], &[0x01, 0x11]);
        assert_eq!(&header[2..4], &[0x00, 0x03]); // OP_REP_IMPORT
        let status = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);

        if status == 0 {
            let mut udev_buf = [0u8; 312];
            self.client.read_exact(&mut udev_buf).await?;
            Ok((status, udev_buf.to_vec()))
        } else {
            Ok((status, vec![]))
        }
    }

    // Send USBIP_CMD_SUBMIT
    pub async fn send_usbip_cmd_submit(
        &mut self,
        seqnum: u32,
        devid: u32,
        direction: u32,
        ep: u32,
        transfer_flags: u32,
        transfer_buffer_length: i32,
        setup: [u8; 8],
        iso_descs: &[UsbipIsoPacketDescriptor],
    ) -> anyhow::Result<()> {
        let mut cmd = Vec::new();
        cmd.extend_from_slice(&0x0001u32.to_be_bytes()); // command: USBIP_CMD_SUBMIT (1)
        cmd.extend_from_slice(&seqnum.to_be_bytes());
        cmd.extend_from_slice(&devid.to_be_bytes());
        cmd.extend_from_slice(&direction.to_be_bytes());
        cmd.extend_from_slice(&ep.to_be_bytes());
        cmd.extend_from_slice(&transfer_flags.to_be_bytes());
        cmd.extend_from_slice(&transfer_buffer_length.to_be_bytes());
        cmd.extend_from_slice(&0i32.to_be_bytes()); // start_frame
        cmd.extend_from_slice(&(iso_descs.len() as i32).to_be_bytes()); // number_of_packets
        cmd.extend_from_slice(&0i32.to_be_bytes()); // interval
        cmd.extend_from_slice(&setup);

        for desc in iso_descs {
            cmd.extend_from_slice(&desc.to_bytes());
        }

        self.client.write_all(&cmd).await?;
        Ok(())
    }

    // Read USBIP_RET_SUBMIT
    // Returns (seqnum, status, actual_len, number_of_packets, payload, returned_iso_descs)
    pub async fn read_usbip_ret_submit(
        &mut self,
    ) -> anyhow::Result<(u32, i32, i32, i32, Vec<u8>, Vec<UsbipIsoPacketDescriptor>)> {
        let mut ret_header = [0u8; 48];
        self.client.read_exact(&mut ret_header).await?;

        let command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
        assert_eq!(command, 0x0003); // USBIP_RET_SUBMIT (3)

        let seqnum = u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
        let status = i32::from_be_bytes([ret_header[20], ret_header[21], ret_header[22], ret_header[23]]);
        let actual_len = i32::from_be_bytes([ret_header[24], ret_header[25], ret_header[26], ret_header[27]]);
        let number_of_packets = i32::from_be_bytes([ret_header[32], ret_header[33], ret_header[34], ret_header[35]]);

        let mut payload = vec![0u8; actual_len.max(0) as usize];
        if actual_len > 0 {
            self.client.read_exact(&mut payload).await?;
        }

        let mut iso_descs = Vec::new();
        if number_of_packets > 0 {
            let mut returned_descs = vec![0u8; (number_of_packets * 16) as usize];
            self.client.read_exact(&mut returned_descs).await?;
            for i in 0..number_of_packets as usize {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&returned_descs[i * 16..(i + 1) * 16]);
                iso_descs.push(UsbipIsoPacketDescriptor::from_bytes(bytes));
            }
        }

        Ok((seqnum, status, actual_len, number_of_packets, payload, iso_descs))
    }

    // Send USBIP_CMD_UNLINK
    pub async fn send_usbip_cmd_unlink(
        &mut self,
        seqnum: u32,
        devid: u32,
        ep: u32,
        unlink_seqnum: u32,
    ) -> anyhow::Result<()> {
        let mut cmd = Vec::new();
        cmd.extend_from_slice(&0x0002u32.to_be_bytes()); // command: USBIP_CMD_UNLINK (2)
        cmd.extend_from_slice(&seqnum.to_be_bytes());
        cmd.extend_from_slice(&devid.to_be_bytes());
        cmd.extend_from_slice(&0u32.to_be_bytes()); // direction (OUT = 0)
        cmd.extend_from_slice(&ep.to_be_bytes());
        cmd.extend_from_slice(&unlink_seqnum.to_be_bytes());
        cmd.extend_from_slice(&[0; 24]); // padding: 24 bytes of zeroes

        self.client.write_all(&cmd).await?;
        Ok(())
    }

    // Read USBIP_RET_UNLINK
    // Returns (seqnum, status)
    pub async fn read_usbip_ret_unlink(&mut self) -> anyhow::Result<(u32, i32)> {
        let mut ret_header = [0u8; 48];
        self.client.read_exact(&mut ret_header).await?;

        let command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
        assert_eq!(command, 0x0004); // USBIP_RET_UNLINK (4)

        let seqnum = u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
        let status = i32::from_be_bytes([ret_header[20], ret_header[21], ret_header[22], ret_header[23]]);

        Ok((seqnum, status))
    }
}

pub struct MockDeviceBuilder {
    pub bus_num: u8,
    pub dev_addr: u8,
    pub dev_speed: UsbSpeed,
    pub descriptor: UsbDeviceDescriptor,
    pub config_descriptor: UsbConfigDescriptor,
    pub transfer_handler: Option<MockTransferCallback>,
    pub dropped: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub open_error: Option<String>,
    pub kernel_drivers: Option<Arc<std::sync::Mutex<std::collections::HashMap<u8, bool>>>>,
    pub claimed_interfaces: Option<Arc<std::sync::Mutex<std::collections::HashSet<u8>>>>,
}

impl Default for MockDeviceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDeviceBuilder {
    pub fn new() -> Self {
        Self {
            bus_num: 1,
            dev_addr: 2,
            dev_speed: UsbSpeed::High,
            descriptor: UsbDeviceDescriptor {
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
            },
            config_descriptor: UsbConfigDescriptor {
                num_interfaces: 1,
                configuration_value: 1,
                max_power: 500,
                self_powered: true,
                remote_wakeup: false,
                interfaces: vec![],
            },
            transfer_handler: None,
            dropped: None,
            open_error: None,
            kernel_drivers: None,
            claimed_interfaces: None,
        }
    }

    pub fn with_transfer_handler(mut self, handler: MockTransferCallback) -> Self {
        self.transfer_handler = Some(handler);
        self
    }

    pub fn with_dropped(mut self, dropped: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.dropped = Some(dropped);
        self
    }

    pub fn with_open_error(mut self, error: String) -> Self {
        self.open_error = Some(error);
        self
    }

    pub fn with_kernel_drivers(mut self, kd: Arc<std::sync::Mutex<std::collections::HashMap<u8, bool>>>) -> Self {
        self.kernel_drivers = Some(kd);
        self
    }

    pub fn with_claimed_interfaces(mut self, ci: Arc<std::sync::Mutex<std::collections::HashSet<u8>>>) -> Self {
        self.claimed_interfaces = Some(ci);
        self
    }

    pub fn build(self) -> Arc<MockUsbDevice> {
        Arc::new(MockUsbDevice {
            bus_num: self.bus_num,
            dev_addr: self.dev_addr,
            dev_speed: self.dev_speed,
            descriptor: self.descriptor,
            config_descriptor: self.config_descriptor,
            transfer_handler: self.transfer_handler,
            dropped: self.dropped,
            open_error: self.open_error,
            kernel_drivers: self.kernel_drivers,
            claimed_interfaces: self.claimed_interfaces,
        })
    }
}
