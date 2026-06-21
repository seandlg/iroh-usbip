use crate::protocol::{
    ST_DEV_BUSY, ST_NA, ST_OK, USBIP_RET_SUBMIT, USBIP_RET_UNLINK, UsbipHeaderBasic,
    UsbipHeaderRetSubmit, UsbipHeaderRetUnlink, UsbipRequest, UsbipResponse, UsbipStream,
};
use crate::{UsbDevice, UsbDeviceHandle};
use std::sync::Arc;

pub async fn run_usbip_session<S, D>(
    stream: S,
    registry: Arc<crate::HostDeviceRegistry<D>>,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    D: UsbDevice + 'static,
{
    let mut ustream = UsbipStream::new(stream);

    let req = match ustream.read_handshake_request().await? {
        Some(req) => req,
        None => return Ok(()),
    };

    match req {
        UsbipRequest::Devlist => {
            let mut details = Vec::new();
            let query = crate::DeviceQuery::default();
            let devices = registry.find_devices(&query)?;
            for entry in devices {
                let detail = crate::protocol::UsbipDeviceDetail::new(entry.device.as_ref())?;
                details.push(detail);
            }
            ustream
                .write_handshake_response(UsbipResponse::Devlist { devices: details })
                .await?;
        }
        UsbipRequest::Import { busid: req_busid } => {
            // Find matching device using registry query
            let query = crate::DeviceQuery {
                bus_id: Some(req_busid),
                ..Default::default()
            };

            if let Ok(entry) = registry.find_single_device(&query) {
                let dev = entry.device;
                let mut inner_handle = match dev.open() {
                    Ok(h) => h,
                    Err(err) => {
                        let status = if err.to_string().contains("busy") {
                            ST_DEV_BUSY
                        } else {
                            ST_NA
                        };
                        ustream
                            .write_handshake_response(UsbipResponse::Import {
                                status,
                                device: None,
                            })
                            .await?;
                        return Ok(());
                    }
                };
                let config =
                    dev.config_descriptor(0)
                        .unwrap_or_else(|_| crate::UsbConfigDescriptor {
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
                    if let Ok(true) = inner_handle.kernel_driver_active(iface_num)
                        && inner_handle.detach_kernel_driver(iface_num).is_ok()
                    {
                        detached_interfaces.push(iface_num);
                    }
                    if inner_handle.claim_interface(iface_num).is_ok() {
                        claimed_interfaces.push(iface_num);
                    }
                }

                let mut handle = DriverGuard {
                    handle: &mut inner_handle,
                    detached_interfaces,
                    claimed_interfaces,
                };

                let detail = crate::protocol::UsbipDeviceDetail::new(dev.as_ref())?;
                ustream
                    .write_handshake_response(UsbipResponse::Import {
                        status: ST_OK,
                        device: Some(Box::new(detail)),
                    })
                    .await?;

                // Transition to Transfer Phase Loop
                let mut runner = TransferRunner::new(dev.as_ref(), &mut *handle);
                while let Some(transfer_req) = ustream.read_transfer_request().await? {
                    let resp = runner.execute(transfer_req)?;
                    ustream.write_transfer_response(resp).await?;
                }
            } else {
                // Not found
                ustream
                    .write_handshake_response(UsbipResponse::Import {
                        status: ST_NA,
                        device: None,
                    })
                    .await?;
            }
        }
        _ => anyhow::bail!("Invalid handshake phase request"),
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
                eprintln!(
                    "Warning: Failed to re-attach kernel driver to interface {}: {}",
                    iface, e
                );
            }
        }
    }
}

pub struct TransferRunner<'a, D: UsbDevice, H: UsbDeviceHandle> {
    device: &'a D,
    handle: &'a mut H,
}

impl<'a, D: UsbDevice, H: UsbDeviceHandle> TransferRunner<'a, D, H> {
    pub fn new(device: &'a D, handle: &'a mut H) -> Self {
        Self { device, handle }
    }

    pub fn execute(&mut self, req: UsbipRequest) -> anyhow::Result<UsbipResponse> {
        match req {
            UsbipRequest::Submit {
                basic,
                submit: cmd_submit,
                data,
                iso_descriptors,
            } => {
                let timeout = std::time::Duration::from_secs(5);
                let transfer_res: (i32, Vec<u8>) = if basic.ep == 0 {
                    let bm_request_type = cmd_submit.setup[0];
                    let b_request = cmd_submit.setup[1];
                    let w_value = u16::from_le_bytes([cmd_submit.setup[2], cmd_submit.setup[3]]);
                    let w_index = u16::from_le_bytes([cmd_submit.setup[4], cmd_submit.setup[5]]);

                    if basic.direction == 1 {
                        // Control Read
                        let mut buf = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                        match self.handle.read_control(
                            bm_request_type,
                            b_request,
                            w_value,
                            w_index,
                            &mut buf,
                            timeout,
                        ) {
                            Ok(len) => {
                                buf.truncate(len);
                                (0, buf)
                            }
                            Err(_) => (-32, vec![]), // EPIPE (-32) on stall/error
                        }
                    } else {
                        // Control Write
                        match self.handle.write_control(
                            bm_request_type,
                            b_request,
                            w_value,
                            w_index,
                            &data,
                            timeout,
                        ) {
                            Ok(_) => (0, vec![]),
                            Err(_) => (-32, vec![]),
                        }
                    }
                } else {
                    let ep_addr = (basic.ep as u8) | if basic.direction == 1 { 0x80 } else { 0x00 };

                    let mut is_interrupt = false;
                    if let Ok(cfg) = self.device.config_descriptor(0) {
                        'outer: for interface in cfg.interfaces {
                            for setting in interface.settings {
                                for endpoint in setting.endpoints {
                                    if endpoint.address == ep_addr {
                                        if endpoint.transfer_type
                                            == crate::UsbTransferType::Interrupt
                                        {
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
                            let mut buf =
                                vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                            match self.handle.read_interrupt(ep_addr, &mut buf, timeout) {
                                Ok(len) => {
                                    buf.truncate(len);
                                    (0, buf)
                                }
                                Err(_) => (-32, vec![]),
                            }
                        } else {
                            match self.handle.write_interrupt(ep_addr, &data, timeout) {
                                Ok(_) => (0, vec![]),
                                Err(_) => (-32, vec![]),
                            }
                        }
                    } else {
                        // Default to Bulk
                        if basic.direction == 1 {
                            let mut buf =
                                vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                            match self.handle.read_bulk(ep_addr, &mut buf, timeout) {
                                Ok(len) => {
                                    buf.truncate(len);
                                    (0, buf)
                                }
                                Err(_) => (-32, vec![]),
                            }
                        } else {
                            match self.handle.write_bulk(ep_addr, &data, timeout) {
                                Ok(_) => (0, vec![]),
                                Err(_) => (-32, vec![]),
                            }
                        }
                    }
                };

                let (status, resp_data) = transfer_res;

                let resp_submit = UsbipHeaderRetSubmit {
                    status,
                    actual_length: resp_data.len() as i32,
                    start_frame: 0,
                    number_of_packets: cmd_submit.number_of_packets,
                    error_count: 0,
                };

                Ok(UsbipResponse::Submit {
                    basic: UsbipHeaderBasic {
                        command: USBIP_RET_SUBMIT,
                        seqnum: basic.seqnum,
                        devid: basic.devid,
                        direction: basic.direction,
                        ep: basic.ep,
                    },
                    submit: resp_submit,
                    data: resp_data,
                    iso_descriptors,
                })
            }
            UsbipRequest::Unlink { basic, unlink: _ } => {
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
                Ok(UsbipResponse::Unlink {
                    basic: resp_basic,
                    unlink: resp_unlink,
                })
            }
            _ => anyhow::bail!("Invalid transfer phase request: {:?}", req),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{USBIP_CMD_SUBMIT, UsbipHeaderBasic, UsbipHeaderCmdSubmit};
    use crate::{MockUsbDevice, UsbConfigDescriptor, UsbDeviceDescriptor, UsbSpeed};
    use std::sync::Arc;

    #[test]
    fn test_transfer_runner_control_read() {
        let desc = UsbDeviceDescriptor {
            vendor_id: 0x1234,
            product_id: 0x5678,
            device_class: 0,
            device_subclass: 0,
            device_protocol: 0,
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

        // Create mock device with a callback
        let callback = Arc::new(|action: String, _data: Vec<u8>| {
            if action == "control_read:128:6:256:0" {
                Ok(vec![0xAA, 0xBB])
            } else {
                Ok(vec![])
            }
        });

        let dev = MockUsbDevice {
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
            manufacturer: "Mock Manufacturer".to_string(),
            product: "Mock Product".to_string(),
            serial_number: "Mock Serial".to_string(),
        };

        let mut handle = dev.open().unwrap();
        let mut runner = TransferRunner::new(&dev, &mut handle);

        let req = UsbipRequest::Submit {
            basic: UsbipHeaderBasic {
                command: USBIP_CMD_SUBMIT,
                seqnum: 1,
                devid: 2,
                direction: 1, // IN
                ep: 0,        // control endpoint
            },
            submit: UsbipHeaderCmdSubmit {
                transfer_flags: 0,
                transfer_buffer_length: 2,
                start_frame: 0,
                number_of_packets: 0,
                interval: 0,
                setup: [128, 6, 0, 1, 0, 0, 2, 0], // setup request
            },
            data: vec![],
            iso_descriptors: vec![],
        };

        let resp = runner.execute(req).unwrap();
        if let UsbipResponse::Submit {
            basic: _,
            submit,
            data,
            iso_descriptors: _,
        } = resp
        {
            assert_eq!(submit.status, 0);
            assert_eq!(submit.actual_length, 2);
            assert_eq!(data, vec![0xAA, 0xBB]);
        } else {
            panic!("Expected UsbipResponse::Submit");
        }
    }
}
