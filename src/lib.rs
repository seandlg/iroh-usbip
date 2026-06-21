pub mod protocol;
pub mod engine;
pub mod registry;
pub mod vhci;

pub use registry::{HostDeviceRegistry, DeviceQuery, RegistryDevice};
pub use vhci::VhciController;

use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbSpeed {
    Low,
    Full,
    High,
    Super,
    SuperPlus,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbTransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbDeviceDescriptor {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_class: u8,
    pub device_subclass: u8,
    pub device_protocol: u8,
    pub max_packet_size_0: u8,
    pub num_configurations: u8,
    pub usb_version: (u8, u8),
    pub device_version: (u8, u8),
    pub manufacturer_string_index: Option<u8>,
    pub product_string_index: Option<u8>,
    pub serial_number_string_index: Option<u8>,
}

impl From<rusb::DeviceDescriptor> for UsbDeviceDescriptor {
    fn from(desc: rusb::DeviceDescriptor) -> Self {
        let usb_version = desc.usb_version();
        let device_version = desc.device_version();
        Self {
            vendor_id: desc.vendor_id(),
            product_id: desc.product_id(),
            device_class: desc.class_code(),
            device_subclass: desc.sub_class_code(),
            device_protocol: desc.protocol_code(),
            max_packet_size_0: desc.max_packet_size(),
            num_configurations: desc.num_configurations(),
            usb_version: (usb_version.major(), usb_version.minor()),
            device_version: (device_version.major(), device_version.minor()),
            manufacturer_string_index: desc.manufacturer_string_index(),
            product_string_index: desc.product_string_index(),
            serial_number_string_index: desc.serial_number_string_index(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbConfigDescriptor {
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub max_power: u16,
    pub self_powered: bool,
    pub remote_wakeup: bool,
    pub interfaces: Vec<UsbInterfaceDescriptor>,
}

impl From<rusb::ConfigDescriptor> for UsbConfigDescriptor {
    fn from(desc: rusb::ConfigDescriptor) -> Self {
        let mut interfaces = Vec::new();
        for interface in desc.interfaces() {
            let mut settings = Vec::new();
            let mut interface_number = 0;
            for setting in interface.descriptors() {
                interface_number = setting.interface_number();
                let mut endpoints = Vec::new();
                for endpoint in setting.endpoint_descriptors() {
                    endpoints.push(UsbEndpointDescriptor {
                        address: endpoint.address(),
                        direction: match endpoint.direction() {
                            rusb::Direction::In => UsbDirection::In,
                            rusb::Direction::Out => UsbDirection::Out,
                        },
                        transfer_type: match endpoint.transfer_type() {
                            rusb::TransferType::Control => UsbTransferType::Control,
                            rusb::TransferType::Isochronous => UsbTransferType::Isochronous,
                            rusb::TransferType::Bulk => UsbTransferType::Bulk,
                            rusb::TransferType::Interrupt => UsbTransferType::Interrupt,
                        },
                        max_packet_size: endpoint.max_packet_size(),
                        interval: endpoint.interval(),
                    });
                }
                settings.push(UsbInterfaceSettingDescriptor {
                    setting_number: setting.setting_number(),
                    class_code: setting.class_code(),
                    sub_class_code: setting.sub_class_code(),
                    protocol_code: setting.protocol_code(),
                    endpoints,
                });
            }
            interfaces.push(UsbInterfaceDescriptor {
                interface_number,
                settings,
            });
        }
        Self {
            num_interfaces: desc.num_interfaces(),
            configuration_value: desc.number(),
            max_power: desc.max_power(),
            self_powered: desc.self_powered(),
            remote_wakeup: desc.remote_wakeup(),
            interfaces,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbInterfaceDescriptor {
    pub interface_number: u8,
    pub settings: Vec<UsbInterfaceSettingDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbInterfaceSettingDescriptor {
    pub setting_number: u8,
    pub class_code: u8,
    pub sub_class_code: u8,
    pub protocol_code: u8,
    pub endpoints: Vec<UsbEndpointDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbEndpointDescriptor {
    pub address: u8,
    pub direction: UsbDirection,
    pub transfer_type: UsbTransferType,
    pub max_packet_size: u16,
    pub interval: u8,
}

pub trait UsbDevice: Send + Sync {
    type Handle: UsbDeviceHandle;

    fn bus_number(&self) -> u8;
    fn address(&self) -> u8;
    fn speed(&self) -> UsbSpeed;
    fn device_descriptor(&self) -> anyhow::Result<UsbDeviceDescriptor>;
    fn config_descriptor(&self, index: u8) -> anyhow::Result<UsbConfigDescriptor>;
    fn open(&self) -> anyhow::Result<Self::Handle>;
}

pub trait UsbDeviceHandle: Send + Sync {
    fn active_configuration(&self) -> anyhow::Result<u8>;
    fn set_active_configuration(&mut self, config: u8) -> anyhow::Result<()>;
    fn claim_interface(&mut self, interface: u8) -> anyhow::Result<()>;
    fn release_interface(&mut self, interface: u8) -> anyhow::Result<()>;
    fn set_alternate_setting(&mut self, interface: u8, setting: u8) -> anyhow::Result<()>;
    fn detach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()>;
    fn attach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()>;
    fn kernel_driver_active(&self, interface: u8) -> anyhow::Result<bool>;

    fn read_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize>;
    fn write_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, data: &[u8], timeout: Duration) -> anyhow::Result<usize>;
    
    fn read_bulk(&mut self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize>;
    fn write_bulk(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> anyhow::Result<usize>;
    
    fn read_interrupt(&mut self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize>;
    fn write_interrupt(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> anyhow::Result<usize>;

    fn read_manufacturer_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String>;
    fn read_product_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String>;
    fn read_serial_number_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String>;
}

pub struct PhysicalUsbDevice {
    device: rusb::Device<rusb::GlobalContext>,
}

impl PhysicalUsbDevice {
    pub fn new(device: rusb::Device<rusb::GlobalContext>) -> Self {
        Self { device }
    }
}

pub struct PhysicalUsbDeviceHandle {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
}

impl UsbDevice for PhysicalUsbDevice {
    type Handle = PhysicalUsbDeviceHandle;

    fn bus_number(&self) -> u8 {
        self.device.bus_number()
    }

    fn address(&self) -> u8 {
        self.device.address()
    }

    fn speed(&self) -> UsbSpeed {
        match self.device.speed() {
            rusb::Speed::Low => UsbSpeed::Low,
            rusb::Speed::Full => UsbSpeed::Full,
            rusb::Speed::High => UsbSpeed::High,
            rusb::Speed::Super => UsbSpeed::Super,
            rusb::Speed::SuperPlus => UsbSpeed::SuperPlus,
            _ => UsbSpeed::Unknown,
        }
    }

    fn device_descriptor(&self) -> anyhow::Result<UsbDeviceDescriptor> {
        let desc = self.device.device_descriptor()?;
        Ok(desc.into())
    }

    fn config_descriptor(&self, index: u8) -> anyhow::Result<UsbConfigDescriptor> {
        let desc = self.device.config_descriptor(index)?;
        Ok(desc.into())
    }

    fn open(&self) -> anyhow::Result<Self::Handle> {
        let handle = self.device.open()?;
        Ok(PhysicalUsbDeviceHandle { handle })
    }
}

impl UsbDeviceHandle for PhysicalUsbDeviceHandle {
    fn active_configuration(&self) -> anyhow::Result<u8> {
        Ok(self.handle.active_configuration()?)
    }

    fn set_active_configuration(&mut self, config: u8) -> anyhow::Result<()> {
        Ok(self.handle.set_active_configuration(config)?)
    }

    fn claim_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        Ok(self.handle.claim_interface(interface)?)
    }

    fn release_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        Ok(self.handle.release_interface(interface)?)
    }

    fn set_alternate_setting(&mut self, interface: u8, setting: u8) -> anyhow::Result<()> {
        Ok(self.handle.set_alternate_setting(interface, setting)?)
    }

    fn detach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        Ok(self.handle.detach_kernel_driver(interface)?)
    }

    fn attach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        Ok(self.handle.attach_kernel_driver(interface)?)
    }

    fn kernel_driver_active(&self, interface: u8) -> anyhow::Result<bool> {
        Ok(self.handle.kernel_driver_active(interface)?)
    }

    fn read_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.read_control(request_type, request, value, index, buf, timeout)?)
    }

    fn write_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, data: &[u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.write_control(request_type, request, value, index, data, timeout)?)
    }

    fn read_bulk(&mut self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.read_bulk(endpoint, buf, timeout)?)
    }

    fn write_bulk(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.write_bulk(endpoint, data, timeout)?)
    }

    fn read_interrupt(&mut self, endpoint: u8, buf: &mut [u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.read_interrupt(endpoint, buf, timeout)?)
    }

    fn write_interrupt(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> anyhow::Result<usize> {
        Ok(self.handle.write_interrupt(endpoint, data, timeout)?)
    }

    fn read_manufacturer_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc.manufacturer_string_index.ok_or_else(|| anyhow::anyhow!("No manufacturer string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }

    fn read_product_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc.product_string_index.ok_or_else(|| anyhow::anyhow!("No product string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }

    fn read_serial_number_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc.serial_number_string_index.ok_or_else(|| anyhow::anyhow!("No serial number string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }
}

pub type MockTransferCallback = Arc<dyn Fn(String, Vec<u8>) -> anyhow::Result<Vec<u8>> + Send + Sync>;

pub struct MockUsbDevice {
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

pub struct MockUsbDeviceHandle {
    pub active_config: u8,
    pub claimed_interfaces: std::collections::HashSet<u8>,
    pub kernel_drivers_active: std::collections::HashMap<u8, bool>,
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
    pub transfer_handler: Option<MockTransferCallback>,
    pub dropped: Option<Arc<std::sync::atomic::AtomicBool>>,
    pub shared_kernel_drivers: Option<Arc<std::sync::Mutex<std::collections::HashMap<u8, bool>>>>,
    pub shared_claimed_interfaces: Option<Arc<std::sync::Mutex<std::collections::HashSet<u8>>>>,
}

impl Drop for MockUsbDeviceHandle {
    fn drop(&mut self) {
        if let Some(ref flag) = self.dropped {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl UsbDevice for MockUsbDevice {
    type Handle = MockUsbDeviceHandle;

    fn bus_number(&self) -> u8 {
        self.bus_num
    }

    fn address(&self) -> u8 {
        self.dev_addr
    }

    fn speed(&self) -> UsbSpeed {
        self.dev_speed
    }

    fn device_descriptor(&self) -> anyhow::Result<UsbDeviceDescriptor> {
        Ok(self.descriptor.clone())
    }

    fn config_descriptor(&self, _index: u8) -> anyhow::Result<UsbConfigDescriptor> {
        Ok(self.config_descriptor.clone())
    }

    fn open(&self) -> anyhow::Result<Self::Handle> {
        if let Some(ref err_msg) = self.open_error {
            return Err(anyhow::anyhow!("{}", err_msg));
        }
        let mut kernel_drivers_active = std::collections::HashMap::new();
        if let Some(ref shared) = self.kernel_drivers {
            kernel_drivers_active = shared.lock().unwrap().clone();
        }
        let mut claimed_interfaces = std::collections::HashSet::new();
        if let Some(ref shared) = self.claimed_interfaces {
            claimed_interfaces = shared.lock().unwrap().clone();
        }
        Ok(MockUsbDeviceHandle {
            active_config: 1,
            claimed_interfaces,
            kernel_drivers_active,
            manufacturer: "Mock Manufacturer".to_string(),
            product: "Mock Product".to_string(),
            serial_number: "Mock Serial".to_string(),
            transfer_handler: self.transfer_handler.clone(),
            dropped: self.dropped.clone(),
            shared_kernel_drivers: self.kernel_drivers.clone(),
            shared_claimed_interfaces: self.claimed_interfaces.clone(),
        })
    }
}

impl UsbDeviceHandle for MockUsbDeviceHandle {
    fn active_configuration(&self) -> anyhow::Result<u8> {
        Ok(self.active_config)
    }

    fn set_active_configuration(&mut self, config: u8) -> anyhow::Result<()> {
        self.active_config = config;
        Ok(())
    }

    fn claim_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        self.claimed_interfaces.insert(interface);
        if let Some(ref shared) = self.shared_claimed_interfaces {
            shared.lock().unwrap().insert(interface);
        }
        Ok(())
    }

    fn release_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        self.claimed_interfaces.remove(&interface);
        if let Some(ref shared) = self.shared_claimed_interfaces {
            shared.lock().unwrap().remove(&interface);
        }
        Ok(())
    }

    fn set_alternate_setting(&mut self, _interface: u8, _setting: u8) -> anyhow::Result<()> {
        Ok(())
    }

    fn detach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        self.kernel_drivers_active.insert(interface, false);
        if let Some(ref shared) = self.shared_kernel_drivers {
            shared.lock().unwrap().insert(interface, false);
        }
        Ok(())
    }

    fn attach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        self.kernel_drivers_active.insert(interface, true);
        if let Some(ref shared) = self.shared_kernel_drivers {
            shared.lock().unwrap().insert(interface, true);
        }
        Ok(())
    }

    fn kernel_driver_active(&self, interface: u8) -> anyhow::Result<bool> {
        Ok(*self.kernel_drivers_active.get(&interface).unwrap_or(&true))
    }

    fn read_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, buf: &mut [u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("control_read:{}:{}:{}:{}", request_type, request, value, index);
            let res = handler(action, vec![])?;
            let len = res.len().min(buf.len());
            buf[..len].copy_from_slice(&res[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn write_control(&mut self, request_type: u8, request: u8, value: u16, index: u16, data: &[u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("control_write:{}:{}:{}:{}", request_type, request, value, index);
            let res = handler(action, data.to_vec())?;
            Ok(res.len())
        } else {
            Ok(0)
        }
    }

    fn read_bulk(&mut self, endpoint: u8, buf: &mut [u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("bulk_read:{}", endpoint);
            let res = handler(action, vec![])?;
            let len = res.len().min(buf.len());
            buf[..len].copy_from_slice(&res[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn write_bulk(&mut self, endpoint: u8, data: &[u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("bulk_write:{}", endpoint);
            let res = handler(action, data.to_vec())?;
            Ok(res.len())
        } else {
            Ok(0)
        }
    }

    fn read_interrupt(&mut self, endpoint: u8, buf: &mut [u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("interrupt_read:{}", endpoint);
            let res = handler(action, vec![])?;
            let len = res.len().min(buf.len());
            buf[..len].copy_from_slice(&res[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn write_interrupt(&mut self, endpoint: u8, data: &[u8], _timeout: Duration) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("interrupt_write:{}", endpoint);
            let res = handler(action, data.to_vec())?;
            Ok(res.len())
        } else {
            Ok(0)
        }
    }

    fn read_manufacturer_string(&self, _desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        Ok(self.manufacturer.clone())
    }

    fn read_product_string(&self, _desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        Ok(self.product.clone())
    }

    fn read_serial_number_string(&self, _desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        Ok(self.serial_number.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_device() {
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
        let dev = MockUsbDevice {
            bus_num: 1,
            dev_addr: 2,
            dev_speed: UsbSpeed::High,
            descriptor: desc.clone(),
            config_descriptor: config,
            transfer_handler: None,
            dropped: None,
            open_error: None,
            kernel_drivers: None,
            claimed_interfaces: None,
        };

        assert_eq!(dev.bus_number(), 1);
        assert_eq!(dev.address(), 2);
        assert_eq!(dev.speed(), UsbSpeed::High);
        assert_eq!(dev.device_descriptor().unwrap(), desc);

        let handle = dev.open().unwrap();
        assert_eq!(handle.active_configuration().unwrap(), 1);
        assert_eq!(handle.read_manufacturer_string(&desc).unwrap(), "Mock Manufacturer");
        assert_eq!(handle.read_product_string(&desc).unwrap(), "Mock Product");
        assert_eq!(handle.read_serial_number_string(&desc).unwrap(), "Mock Serial");
    }
}
