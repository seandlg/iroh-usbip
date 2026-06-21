use crate::{AnyUsbDevice, PhysicalUsbDevice, UsbDevice, UsbDeviceHandle};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceQuery {
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub bus_number: Option<u8>,
    pub address: Option<u8>,
    pub bus_id: Option<String>,
}

impl DeviceQuery {
    pub fn from_cli_args(
        vid: Option<&str>,
        pid: Option<&str>,
        bus_number: Option<u8>,
        address: Option<u8>,
    ) -> anyhow::Result<Self> {
        let vendor_id = vid
            .map(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16))
            .transpose()?;
        let product_id = pid
            .map(|s| u16::from_str_radix(s.trim_start_matches("0x"), 16))
            .transpose()?;

        Ok(Self {
            vendor_id,
            product_id,
            bus_number,
            address,
            bus_id: None,
        })
    }
}

impl DeviceQuery {
    pub fn matches<D: UsbDevice + ?Sized>(&self, device: &D) -> bool {
        if let Some(vid) = self.vendor_id {
            if let Ok(desc) = device.device_descriptor() {
                if desc.vendor_id != vid {
                    return false;
                }
            } else {
                return false;
            }
        }
        if let Some(pid) = self.product_id {
            if let Ok(desc) = device.device_descriptor() {
                if desc.product_id != pid {
                    return false;
                }
            } else {
                return false;
            }
        }
        if let Some(bus_num) = self.bus_number
            && device.bus_number() != bus_num
        {
            return false;
        }
        if let Some(address) = self.address
            && device.address() != address
        {
            return false;
        }
        if let Some(ref bus_id) = self.bus_id {
            let actual = format!("{}-{}", device.bus_number(), device.address());
            if &actual != bus_id {
                return false;
            }
        }
        true
    }
}

#[derive(Clone)]
pub struct RegistryDevice<D: UsbDevice + ?Sized> {
    pub device: Arc<D>,
    pub bus_id: String,
    pub sysfs_path: PathBuf,
    pub cli_info: String,
}

impl<D: UsbDevice + ?Sized> RegistryDevice<D> {
    pub fn new(device: Arc<D>) -> Self {
        let bus = device.bus_number();
        let addr = device.address();
        let bus_id = HostDeviceRegistry::<D>::bus_id(bus, addr);
        let sysfs_path = HostDeviceRegistry::<D>::sysfs_path(bus, addr);
        let cli_info = HostDeviceRegistry::<D>::format_cli_info(&device);
        Self {
            device,
            bus_id,
            sysfs_path,
            cli_info,
        }
    }
}

pub struct HostDeviceRegistry<D: UsbDevice + ?Sized> {
    devices: Vec<Arc<D>>,
}

impl HostDeviceRegistry<AnyUsbDevice> {
    pub fn new_physical() -> anyhow::Result<Self> {
        let mut devices = Vec::new();
        for dev in rusb::devices()?.iter() {
            devices.push(Arc::new(AnyUsbDevice::Physical(PhysicalUsbDevice::new(
                dev,
            ))));
        }
        Ok(Self { devices })
    }

    pub fn new_mock() -> anyhow::Result<Self> {
        let desc = crate::UsbDeviceDescriptor {
            vendor_id: 0x1d6b,
            product_id: 0x0104,
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
        let config = crate::UsbConfigDescriptor {
            num_interfaces: 1,
            configuration_value: 1,
            max_power: 500,
            self_powered: true,
            remote_wakeup: false,
            interfaces: vec![crate::UsbInterfaceDescriptor {
                interface_number: 0,
                settings: vec![crate::UsbInterfaceSettingDescriptor {
                    setting_number: 0,
                    class_code: 0x0a,
                    sub_class_code: 0,
                    protocol_code: 0,
                    endpoints: vec![],
                }],
            }],
        };
        let mock_dev = crate::MockUsbDevice {
            bus_num: 1,
            dev_addr: 2,
            dev_speed: crate::UsbSpeed::High,
            descriptor: desc,
            config_descriptor: config,
            transfer_handler: Some(Arc::new(|action, _data| {
                if action == "control_read:128:6:256:0" {
                    let descriptor_bytes = vec![
                        18, 1, 0x00, 0x02, 0, 0, 0, 64, 0x6b, 0x1d, 0x04, 0x01, 0x00, 0x01, 1, 2,
                        3, 1,
                    ];
                    return Ok(descriptor_bytes);
                }
                Ok(vec![])
            })),
            dropped: None,
            open_error: None,
            kernel_drivers: None,
            claimed_interfaces: None,
            manufacturer: "Antigravity".to_string(),
            product: "E2E_Virtual_Gadget".to_string(),
            serial_number: "0123456789".to_string(),
        };

        Ok(Self {
            devices: vec![Arc::new(AnyUsbDevice::Mock(mock_dev))],
        })
    }
}

impl<D: UsbDevice + ?Sized> HostDeviceRegistry<D> {
    pub fn new_static(devices: Vec<Arc<D>>) -> Self {
        Self { devices }
    }

    pub fn sysfs_path(bus: u8, address: u8) -> PathBuf {
        PathBuf::from(format!("/sys/devices/mock/usb{}/{}-{}", bus, bus, address))
    }

    pub fn bus_id(bus: u8, address: u8) -> String {
        format!("{}-{}", bus, address)
    }

    pub fn format_cli_info(device: &D) -> String {
        let bus = device.bus_number();
        let addr = device.address();
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => {
                return format!("Bus {:03} Device {:03}: ID <unknown>", bus, addr);
            }
        };

        let mut manufacturer = None;
        let mut product = None;

        match device.open() {
            Ok(handle) => {
                if desc.manufacturer_string_index.is_some() {
                    manufacturer = handle.read_manufacturer_string(&desc).ok();
                }
                if desc.product_string_index.is_some() {
                    product = handle.read_product_string(&desc).ok();
                }
            }
            Err(_) => {
                if desc.manufacturer_string_index.is_some() {
                    manufacturer = Some("<Access Denied>".to_string());
                }
                if desc.product_string_index.is_some() {
                    product = Some("<Access Denied>".to_string());
                }
            }
        }

        let details = match (manufacturer, product) {
            (Some(m), Some(p)) => format!("{} {}", m, p),
            (Some(m), None) => m,
            (None, Some(p)) => p,
            (None, None) => String::new(),
        };

        if details.is_empty() {
            format!(
                "Bus {:03} Device {:03}: ID {:04x}:{:04x}",
                bus, addr, desc.vendor_id, desc.product_id
            )
        } else {
            format!(
                "Bus {:03} Device {:03}: ID {:04x}:{:04x} {}",
                bus, addr, desc.vendor_id, desc.product_id, details
            )
        }
    }

    pub fn find_devices(&self, query: &DeviceQuery) -> anyhow::Result<Vec<RegistryDevice<D>>> {
        let mut matched = Vec::new();
        for dev in &self.devices {
            if query.matches(dev.as_ref()) {
                matched.push(RegistryDevice::new(Arc::clone(dev)));
            }
        }
        Ok(matched)
    }

    pub fn find_single_device(&self, query: &DeviceQuery) -> anyhow::Result<RegistryDevice<D>> {
        let matched = self.find_devices(query)?;
        if matched.is_empty() {
            anyhow::bail!("No matching USB device found.");
        } else if matched.len() > 1 {
            anyhow::bail!("Multiple matching USB devices found. Please specify more filters.");
        } else {
            Ok(matched.into_iter().next().unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockUsbDevice, UsbConfigDescriptor, UsbDeviceDescriptor, UsbSpeed};

    #[test]
    fn test_from_cli_args_parsing() {
        let query =
            DeviceQuery::from_cli_args(Some("0x1d6b"), Some("0002"), Some(1), Some(2)).unwrap();
        assert_eq!(query.vendor_id, Some(0x1d6b));
        assert_eq!(query.product_id, Some(0x0002));
        assert_eq!(query.bus_number, Some(1));
        assert_eq!(query.address, Some(2));
    }

    fn make_mock_device(
        bus_num: u8,
        dev_addr: u8,
        vendor_id: u16,
        product_id: u16,
    ) -> Arc<MockUsbDevice> {
        Arc::new(MockUsbDevice {
            bus_num,
            dev_addr,
            dev_speed: UsbSpeed::High,
            descriptor: UsbDeviceDescriptor {
                vendor_id,
                product_id,
                device_class: 0,
                device_subclass: 0,
                device_protocol: 0,
                max_packet_size_0: 64,
                num_configurations: 1,
                usb_version: (2, 0),
                device_version: (1, 0),
                manufacturer_string_index: None,
                product_string_index: None,
                serial_number_string_index: None,
            },
            config_descriptor: UsbConfigDescriptor {
                num_interfaces: 0,
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
            manufacturer: "Mock Manufacturer".to_string(),
            product: "Mock Product".to_string(),
            serial_number: "Mock Serial".to_string(),
        })
    }

    #[test]
    fn test_registry_matching() {
        let dev1 = make_mock_device(1, 2, 0x1234, 0x5678);
        let dev2 = make_mock_device(1, 3, 0x1234, 0xabcd);
        let dev3 = make_mock_device(2, 1, 0x9999, 0x9999);

        let registry = HostDeviceRegistry::new_static(vec![dev1, dev2, dev3]);

        // 1. Query matching vendor ID
        let query_vid = DeviceQuery {
            vendor_id: Some(0x1234),
            ..Default::default()
        };
        let matches = registry.find_devices(&query_vid).unwrap();
        assert_eq!(matches.len(), 2);

        // 2. Query matching single device uniquely
        let query_unique = DeviceQuery {
            vendor_id: Some(0x1234),
            address: Some(3),
            ..Default::default()
        };
        let unique = registry.find_single_device(&query_unique).unwrap();
        assert_eq!(unique.device.bus_number(), 1);
        assert_eq!(unique.device.address(), 3);

        // 3. Query matching 0 devices
        let query_none = DeviceQuery {
            vendor_id: Some(0x8888),
            ..Default::default()
        };
        assert!(registry.find_single_device(&query_none).is_err());

        // 4. Query matching multiple devices (ambiguity error)
        assert!(registry.find_single_device(&query_vid).is_err());
    }
}
