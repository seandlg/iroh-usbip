use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::{UsbDevice, UsbDeviceHandle};
use crate::protocol::{
    OP_REQ_DEVLIST, OP_REP_DEVLIST, OP_REQ_IMPORT, OP_REP_IMPORT,
    USBIP_CMD_SUBMIT, USBIP_RET_SUBMIT, USBIP_CMD_UNLINK,
    USBIP_VERSION, pad_string, map_speed,
    UsbipUsbDevice, UsbipUsbInterface,
};

pub async fn run_usbip_session<S, D>(mut stream: S, devices: Vec<Arc<D>>) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    D: UsbDevice + 'static,
{
    // Read the 8-byte op_common header
    let mut header = [0u8; 8];
    if let Err(_) = stream.read_exact(&mut header).await {
        // Stream closed or failed to read header
        return Ok(());
    }

    let version = u16::from_be_bytes([header[0], header[1]]);
    let code = u16::from_be_bytes([header[2], header[3]]);
    let _status = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);

    if version != USBIP_VERSION {
        anyhow::bail!("Unsupported USBIP version: {:04x}", version);
    }

    match code {
        OP_REQ_DEVLIST => {
            // Respond with OP_REP_DEVLIST
            let mut response = Vec::new();
            response.extend_from_slice(&USBIP_VERSION.to_be_bytes());
            response.extend_from_slice(&OP_REP_DEVLIST.to_be_bytes());
            response.extend_from_slice(&0u32.to_be_bytes());
            response.extend_from_slice(&(devices.len() as u32).to_be_bytes());

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
            // Read 32 bytes of busid
            let mut busid_buf = [0u8; 32];
            stream.read_exact(&mut busid_buf).await?;
            let req_busid = std::str::from_utf8(&busid_buf)?
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
                let mut handle = dev.open()?;
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

                let mut response = Vec::new();
                response.extend_from_slice(&USBIP_VERSION.to_be_bytes());
                response.extend_from_slice(&OP_REP_IMPORT.to_be_bytes());
                response.extend_from_slice(&0u32.to_be_bytes());
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

                    let command = u32::from_be_bytes([cmd_buf[0], cmd_buf[1], cmd_buf[2], cmd_buf[3]]);
                    let seqnum = u32::from_be_bytes([cmd_buf[4], cmd_buf[5], cmd_buf[6], cmd_buf[7]]);
                    let devid = u32::from_be_bytes([cmd_buf[8], cmd_buf[9], cmd_buf[10], cmd_buf[11]]);
                    let direction = u32::from_be_bytes([cmd_buf[12], cmd_buf[13], cmd_buf[14], cmd_buf[15]]);
                    let ep = u32::from_be_bytes([cmd_buf[16], cmd_buf[17], cmd_buf[18], cmd_buf[19]]);

                    match command {
                        USBIP_CMD_SUBMIT => {
                            let _transfer_flags = u32::from_be_bytes([cmd_buf[20], cmd_buf[21], cmd_buf[22], cmd_buf[23]]);
                            let transfer_buffer_length = i32::from_be_bytes([cmd_buf[24], cmd_buf[25], cmd_buf[26], cmd_buf[27]]);
                            let _start_frame = i32::from_be_bytes([cmd_buf[28], cmd_buf[29], cmd_buf[30], cmd_buf[31]]);
                            let _number_of_packets = i32::from_be_bytes([cmd_buf[32], cmd_buf[33], cmd_buf[34], cmd_buf[35]]);
                            let _interval = i32::from_be_bytes([cmd_buf[36], cmd_buf[37], cmd_buf[38], cmd_buf[39]]);
                            let setup = &cmd_buf[40..48];

                            // Read OUT data if direction is OUT (0)
                            let mut data = vec![0u8; transfer_buffer_length.max(0) as usize];
                            if direction == 0 && transfer_buffer_length > 0 {
                                stream.read_exact(&mut data).await?;
                            }

                            let timeout = std::time::Duration::from_secs(5);
                            let transfer_res: (i32, Vec<u8>) = if ep == 0 {
                                let bm_request_type = setup[0];
                                let b_request = setup[1];
                                let w_value = u16::from_le_bytes([setup[2], setup[3]]);
                                let w_index = u16::from_le_bytes([setup[4], setup[5]]);

                                if direction == 1 {
                                    // Control Read
                                    let mut buf = vec![0u8; transfer_buffer_length.max(0) as usize];
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
                                let ep_addr = (ep as u8) | if direction == 1 { 0x80 } else { 0x00 };

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
                                    if direction == 1 {
                                        let mut buf = vec![0u8; transfer_buffer_length.max(0) as usize];
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
                                    if direction == 1 {
                                        let mut buf = vec![0u8; transfer_buffer_length.max(0) as usize];
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

                            let mut resp = Vec::new();
                            resp.extend_from_slice(&USBIP_RET_SUBMIT.to_be_bytes());
                            resp.extend_from_slice(&seqnum.to_be_bytes());
                            resp.extend_from_slice(&devid.to_be_bytes());
                            resp.extend_from_slice(&direction.to_be_bytes());
                            resp.extend_from_slice(&ep.to_be_bytes());

                            resp.extend_from_slice(&status.to_be_bytes());
                            resp.extend_from_slice(&(resp_data.len() as i32).to_be_bytes());
                            resp.extend_from_slice(&0i32.to_be_bytes()); // start_frame
                            resp.extend_from_slice(&0i32.to_be_bytes()); // number_of_packets
                            resp.extend_from_slice(&0i32.to_be_bytes()); // error_count
                            resp.extend_from_slice(&[0; 8]); // setup padding

                            if direction == 1 {
                                resp.extend_from_slice(&resp_data);
                            }

                            stream.write_all(&resp).await?;
                            stream.flush().await?;
                        }
                        USBIP_CMD_UNLINK => {
                            let _unlink_seqnum = u32::from_be_bytes([cmd_buf[20], cmd_buf[21], cmd_buf[22], cmd_buf[23]]);

                            let mut resp = Vec::new();
                            resp.extend_from_slice(&crate::protocol::USBIP_RET_UNLINK.to_be_bytes());
                            resp.extend_from_slice(&seqnum.to_be_bytes());
                            resp.extend_from_slice(&devid.to_be_bytes());
                            resp.extend_from_slice(&direction.to_be_bytes());
                            resp.extend_from_slice(&ep.to_be_bytes());

                            resp.extend_from_slice(&(-104i32).to_be_bytes());
                            resp.extend_from_slice(&[0; 24]);

                            stream.write_all(&resp).await?;
                            stream.flush().await?;
                        }
                        _ => {
                            anyhow::bail!("Unknown transfer command: {:08x}", command);
                        }
                    }
                }
            } else {
                // Not found
                let mut response = Vec::new();
                response.extend_from_slice(&USBIP_VERSION.to_be_bytes());
                response.extend_from_slice(&OP_REP_IMPORT.to_be_bytes());
                response.extend_from_slice(&1u32.to_be_bytes()); // Status = 1 (error/not found)
                stream.write_all(&response).await?;
                stream.flush().await?;
            }
        }
        _ => {
            anyhow::bail!("Unknown USBIP command code: {:04x}", code);
        }
    }

    Ok(())
}
