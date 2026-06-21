use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::{UsbDevice, UsbDeviceHandle};
use crate::protocol::{
    OP_REQ_DEVLIST, OP_REP_DEVLIST, OP_REQ_IMPORT, OP_REP_IMPORT,
    USBIP_CMD_SUBMIT, USBIP_RET_SUBMIT, USBIP_CMD_UNLINK, USBIP_RET_UNLINK,
    USBIP_VERSION, pad_string, map_speed,
    UsbipUsbDevice, UsbipUsbInterface,
    OpCommon, OpDevlistReply, OpImportRequest, ST_OK, ST_NA, ST_DEV_BUSY,
    UsbipHeaderBasic, UsbipHeaderCmdSubmit, UsbipHeaderRetSubmit,
    UsbipHeaderCmdUnlink, UsbipHeaderRetUnlink, UsbipIsoPacketDescriptor,
};

pub async fn run_usbip_session<S, D>(mut stream: S, devices: Vec<Arc<D>>) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    D: UsbDevice + 'static,
{
    // Read the 8-byte op_common header
    let mut header = [0u8; 8];
    if stream.read_exact(&mut header).await.is_err() {
        // Stream closed or failed to read header
        return Ok(());
    }

    let common = OpCommon::from_bytes(header);

    if common.version != USBIP_VERSION {
        anyhow::bail!("Unsupported USBIP version: {:04x}", common.version);
    }

    match common.code {
        OP_REQ_DEVLIST => {
            // Respond with OP_REP_DEVLIST
            let mut response = Vec::new();
            let rep_header = OpCommon {
                version: USBIP_VERSION,
                code: OP_REP_DEVLIST,
                status: ST_OK,
            };
            response.extend_from_slice(&rep_header.to_bytes());
            
            let rep_devlist = OpDevlistReply {
                ndev: devices.len() as u32,
            };
            response.extend_from_slice(&rep_devlist.to_bytes());

            for dev in &devices {
                let desc = dev.device_descriptor()?;
                let config = dev.config_descriptor(0).unwrap_or_else(|_| crate::UsbConfigDescriptor {
                    num_interfaces: 0,
                    configuration_value: 1,
                    max_power: 500,
                    self_powered: true,
                    remote_wakeup: false,
                    interfaces: vec![],
                });

                let busnum = dev.bus_number();
                let addr = dev.address();

                let path_str = format!("/sys/devices/mock/usb{}/{}-{}", busnum, busnum, addr);
                let path_bytes = pad_string(&path_str, 256);
                let mut path = [0u8; 256];
                path.copy_from_slice(&path_bytes);

                let busid_str = format!("{}-{}", busnum, addr);
                let busid_bytes = pad_string(&busid_str, 32);
                let mut busid = [0u8; 32];
                busid.copy_from_slice(&busid_bytes);

                let udev = UsbipUsbDevice {
                    path,
                    busid,
                    busnum: busnum as u32,
                    devnum: addr as u32,
                    speed: map_speed(dev.speed()),
                    id_vendor: desc.vendor_id,
                    id_product: desc.product_id,
                    bcd_device: ((desc.device_version.0 as u16) << 8) | (desc.device_version.1 as u16),
                    b_device_class: desc.device_class,
                    b_device_subclass: desc.device_subclass,
                    b_device_protocol: desc.device_protocol,
                    b_configuration_value: config.configuration_value,
                    b_num_configurations: desc.num_configurations,
                    b_num_interfaces: config.num_interfaces,
                };

                response.extend_from_slice(&udev.to_bytes());

                for interface in &config.interfaces {
                    let setting = interface.settings.first().cloned().unwrap_or_else(|| {
                        crate::UsbInterfaceSettingDescriptor {
                            setting_number: 0,
                            class_code: 0,
                            sub_class_code: 0,
                            protocol_code: 0,
                            endpoints: vec![],
                        }
                    });

                    let uinf = UsbipUsbInterface {
                        b_interface_class: setting.class_code,
                        b_interface_subclass: setting.sub_class_code,
                        b_interface_protocol: setting.protocol_code,
                        padding: 0,
                    };

                    response.extend_from_slice(&uinf.to_bytes());
                }
            }

            stream.write_all(&response).await?;
            stream.flush().await?;
        }
        OP_REQ_IMPORT => {
            // Read 32 bytes of busid (using OpImportRequest)
            let mut busid_buf = [0u8; 32];
            stream.read_exact(&mut busid_buf).await?;
            let import_req = OpImportRequest::from_bytes(busid_buf);
            let req_busid = std::str::from_utf8(&import_req.busid)?
                .trim_end_matches('\0')
                .to_string();

            // Find matching device
            let mut found_device = None;
            for dev in &devices {
                let busid_str = format!("{}-{}", dev.bus_number(), dev.address());
                if busid_str == req_busid {
                    found_device = Some(dev.clone());
                    break;
                }
            }

            if let Some(dev) = found_device {
                let mut inner_handle = match dev.open() {
                    Ok(h) => h,
                    Err(err) => {
                        let status = if err.to_string().contains("busy") {
                            ST_DEV_BUSY
                        } else {
                            ST_NA
                        };
                        let mut response = Vec::new();
                        let rep_header = OpCommon {
                            version: USBIP_VERSION,
                            code: OP_REP_IMPORT,
                            status,
                        };
                        response.extend_from_slice(&rep_header.to_bytes());
                        stream.write_all(&response).await?;
                        stream.flush().await?;
                        return Ok(());
                    }
                };
                let desc = dev.device_descriptor()?;
                let config = dev.config_descriptor(0).unwrap_or_else(|_| crate::UsbConfigDescriptor {
                    num_interfaces: 0,
                    configuration_value: 1,
                    max_power: 500,
                    self_powered: true,
                    remote_wakeup: false,
                    interfaces: vec![],
                });

                // Detach active host kernel drivers and claim interfaces
                let mut detached_interfaces = Vec::new();
                let mut claimed_interfaces = Vec::new();
                for interface in &config.interfaces {
                    let iface_num = interface.interface_number;
                    if let Ok(true) = inner_handle.kernel_driver_active(iface_num) {
                        if let Ok(_) = inner_handle.detach_kernel_driver(iface_num) {
                            detached_interfaces.push(iface_num);
                        }
                    }
                    if let Ok(_) = inner_handle.claim_interface(iface_num) {
                        claimed_interfaces.push(iface_num);
                    }
                }

                let mut handle = DriverGuard {
                    handle: &mut inner_handle,
                    detached_interfaces,
                    claimed_interfaces,
                };

                let busnum = dev.bus_number();
                let addr = dev.address();

                let path_str = format!("/sys/devices/mock/usb{}/{}-{}", busnum, busnum, addr);
                let path_bytes = pad_string(&path_str, 256);
                let mut path = [0u8; 256];
                path.copy_from_slice(&path_bytes);

                let busid_str = format!("{}-{}", busnum, addr);
                let busid_bytes = pad_string(&busid_str, 32);
                let mut busid = [0u8; 32];
                busid.copy_from_slice(&busid_bytes);

                let udev = UsbipUsbDevice {
                    path,
                    busid,
                    busnum: busnum as u32,
                    devnum: addr as u32,
                    speed: map_speed(dev.speed()),
                    id_vendor: desc.vendor_id,
                    id_product: desc.product_id,
                    bcd_device: ((desc.device_version.0 as u16) << 8) | (desc.device_version.1 as u16),
                    b_device_class: desc.device_class,
                    b_device_subclass: desc.device_subclass,
                    b_device_protocol: desc.device_protocol,
                    b_configuration_value: config.configuration_value,
                    b_num_configurations: desc.num_configurations,
                    b_num_interfaces: config.num_interfaces,
                };

                let mut response = Vec::new();
                let rep_header = OpCommon {
                    version: USBIP_VERSION,
                    code: OP_REP_IMPORT,
                    status: ST_OK,
                };
                response.extend_from_slice(&rep_header.to_bytes());
                response.extend_from_slice(&udev.to_bytes());

                stream.write_all(&response).await?;
                stream.flush().await?;

                // Transition to Transfer Phase Loop
                loop {
                    let mut cmd_buf = [0u8; 48];
                    if let Err(_) = stream.read_exact(&mut cmd_buf).await {
                        // Connection closed by client
                        break;
                    }

                    let mut basic_bytes = [0u8; 20];
                    basic_bytes.copy_from_slice(&cmd_buf[0..20]);
                    let basic = UsbipHeaderBasic::from_bytes(basic_bytes);

                    let mut payload_bytes = [0u8; 28];
                    payload_bytes.copy_from_slice(&cmd_buf[20..48]);

                    match basic.command {
                        USBIP_CMD_SUBMIT => {
                            let cmd_submit = UsbipHeaderCmdSubmit::from_bytes(payload_bytes);

                            // Read OUT data if direction is OUT (0)
                            let mut data = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                            if basic.direction == 0 && cmd_submit.transfer_buffer_length > 0 {
                                stream.read_exact(&mut data).await?;
                            }

                            // Read isochronous packet descriptors if number_of_packets > 0
                            let mut iso_descriptors = Vec::new();
                            if cmd_submit.number_of_packets > 0 {
                                let total_desc_bytes = (cmd_submit.number_of_packets as usize) * 16;
                                let mut desc_buf = vec![0u8; total_desc_bytes];
                                stream.read_exact(&mut desc_buf).await?;
                                for chunk in desc_buf.chunks_exact(16) {
                                    let mut arr = [0u8; 16];
                                    arr.copy_from_slice(chunk);
                                    iso_descriptors.push(UsbipIsoPacketDescriptor::from_bytes(arr));
                                }
                            }

                            let timeout = std::time::Duration::from_secs(5);
                            let transfer_res: (i32, Vec<u8>) = if basic.ep == 0 {
                                let bm_request_type = cmd_submit.setup[0];
                                let b_request = cmd_submit.setup[1];
                                let w_value = u16::from_le_bytes([cmd_submit.setup[2], cmd_submit.setup[3]]);
                                let w_index = u16::from_le_bytes([cmd_submit.setup[4], cmd_submit.setup[5]]);

                                if basic.direction == 1 {
                                    // Control Read
                                    let mut buf = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                                    match handle.read_control(bm_request_type, b_request, w_value, w_index, &mut buf, timeout) {
                                        Ok(len) => {
                                            buf.truncate(len);
                                            (0, buf)
                                        }
                                        Err(_) => (-32, vec![]), // EPIPE (-32) on stall/error
                                    }
                                } else {
                                    // Control Write
                                    match handle.write_control(bm_request_type, b_request, w_value, w_index, &data, timeout) {
                                        Ok(_) => (0, vec![]),
                                        Err(_) => (-32, vec![]),
                                    }
                                }
                            } else {
                                let ep_addr = (basic.ep as u8) | if basic.direction == 1 { 0x80 } else { 0x00 };

                                let mut is_interrupt = false;
                                if let Ok(cfg) = dev.config_descriptor(0) {
                                    'outer: for interface in cfg.interfaces {
                                        for setting in interface.settings {
                                            for endpoint in setting.endpoints {
                                                if endpoint.address == ep_addr {
                                                    if endpoint.transfer_type == crate::UsbTransferType::Interrupt {
                                                        is_interrupt = true;
                                                    }
                                                    break 'outer;
                                                }
                                            }
                                        }
                                    }
                                }

                                if is_interrupt {
                                    if basic.direction == 1 {
                                        let mut buf = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                                        match handle.read_interrupt(ep_addr, &mut buf, timeout) {
                                            Ok(len) => {
                                                buf.truncate(len);
                                                (0, buf)
                                            }
                                            Err(_) => (-32, vec![]),
                                        }
                                    } else {
                                        match handle.write_interrupt(ep_addr, &data, timeout) {
                                            Ok(_) => (0, vec![]),
                                            Err(_) => (-32, vec![]),
                                        }
                                    }
                                } else {
                                    // Default to Bulk
                                    if basic.direction == 1 {
                                        let mut buf = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                                        match handle.read_bulk(ep_addr, &mut buf, timeout) {
                                            Ok(len) => {
                                                buf.truncate(len);
                                                (0, buf)
                                            }
                                            Err(_) => (-32, vec![]),
                                        }
                                    } else {
                                        match handle.write_bulk(ep_addr, &data, timeout) {
                                            Ok(_) => (0, vec![]),
                                            Err(_) => (-32, vec![]),
                                        }
                                    }
                                }
                            };

                            let (status, resp_data) = transfer_res;

                            let resp_basic = UsbipHeaderBasic {
                                command: USBIP_RET_SUBMIT,
                                seqnum: basic.seqnum,
                                devid: basic.devid,
                                direction: basic.direction,
                                ep: basic.ep,
                            };
                            let resp_submit = UsbipHeaderRetSubmit {
                                status,
                                actual_length: resp_data.len() as i32,
                                start_frame: 0,
                                number_of_packets: cmd_submit.number_of_packets,
                                error_count: 0,
                            };
                            let mut resp = [0u8; 48];
                            resp[0..20].copy_from_slice(&resp_basic.to_bytes());
                            resp[20..48].copy_from_slice(&resp_submit.to_bytes());

                            stream.write_all(&resp).await?;
                            if basic.direction == 1 {
                                stream.write_all(&resp_data).await?;
                            }

                            // Write back dummy descriptors if number_of_packets > 0 and IN transfer
                            if basic.direction == 1 && cmd_submit.number_of_packets > 0 {
                                let mut dummy_desc_bytes = Vec::with_capacity(iso_descriptors.len() * 16);
                                for desc in &iso_descriptors {
                                    let dummy_desc = UsbipIsoPacketDescriptor {
                                        offset: desc.offset,
                                        length: desc.length,
                                        actual_length: desc.length,
                                        status: 0,
                                    };
                                    dummy_desc_bytes.extend_from_slice(&dummy_desc.to_bytes());
                                }
                                stream.write_all(&dummy_desc_bytes).await?;
                            }

                            stream.flush().await?;
                        }
                        USBIP_CMD_UNLINK => {
                            let _cmd_unlink = UsbipHeaderCmdUnlink::from_bytes(payload_bytes);

                            let resp_basic = UsbipHeaderBasic {
                                command: USBIP_RET_UNLINK,
                                seqnum: basic.seqnum,
                                devid: basic.devid,
                                direction: basic.direction,
                                ep: basic.ep,
                            };
                            let resp_unlink = UsbipHeaderRetUnlink {
                                status: -104, // -ECONNRESET
                            };
                            let mut resp = [0u8; 48];
                            resp[0..20].copy_from_slice(&resp_basic.to_bytes());
                            resp[20..48].copy_from_slice(&resp_unlink.to_bytes());

                            stream.write_all(&resp).await?;
                            stream.flush().await?;
                        }
                        _ => {
                            anyhow::bail!("Unknown transfer command: {:08x}", basic.command);
                        }
                    }
                }
            } else {
                // Not found
                let mut response = Vec::new();
                let rep_header = OpCommon {
                    version: USBIP_VERSION,
                    code: OP_REP_IMPORT,
                    status: ST_NA,
                };
                response.extend_from_slice(&rep_header.to_bytes());
                stream.write_all(&response).await?;
                stream.flush().await?;
            }
        }
        _ => {
            anyhow::bail!("Unknown USBIP command code: {:04x}", common.code);
        }
    }

    Ok(())
}

struct DriverGuard<'a, H: crate::UsbDeviceHandle> {
    handle: &'a mut H,
    detached_interfaces: Vec<u8>,
    claimed_interfaces: Vec<u8>,
}

impl<'a, H: crate::UsbDeviceHandle> std::ops::Deref for DriverGuard<'a, H> {
    type Target = H;
    fn deref(&self) -> &Self::Target {
        self.handle
    }
}

impl<'a, H: crate::UsbDeviceHandle> std::ops::DerefMut for DriverGuard<'a, H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.handle
    }
}

impl<'a, H: crate::UsbDeviceHandle> Drop for DriverGuard<'a, H> {
    fn drop(&mut self) {
        for &iface in &self.claimed_interfaces {
            if let Err(e) = self.handle.release_interface(iface) {
                eprintln!("Warning: Failed to release interface {}: {}", iface, e);
            }
        }
        for &iface in &self.detached_interfaces {
            if let Err(e) = self.handle.attach_kernel_driver(iface) {
                eprintln!("Warning: Failed to re-attach kernel driver to interface {}: {}", iface, e);
            }
        }
    }
}
