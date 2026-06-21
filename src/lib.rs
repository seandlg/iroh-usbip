pub mod engine;
pub mod protocol;
pub mod registry;
pub mod vhci;

pub use registry::{DeviceQuery, HostDeviceRegistry, RegistryDevice};
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

    fn read_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize>;
    fn write_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize>;

    fn read_bulk(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize>;
    fn write_bulk(&mut self, endpoint: u8, data: &[u8], timeout: Duration)
    -> anyhow::Result<usize>;

    fn read_interrupt(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize>;
    fn write_interrupt(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize>;

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

    fn read_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self
            .handle
            .read_control(request_type, request, value, index, buf, timeout)?)
    }

    fn write_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self
            .handle
            .write_control(request_type, request, value, index, data, timeout)?)
    }

    fn read_bulk(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self.handle.read_bulk(endpoint, buf, timeout)?)
    }

    fn write_bulk(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self.handle.write_bulk(endpoint, data, timeout)?)
    }

    fn read_interrupt(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self.handle.read_interrupt(endpoint, buf, timeout)?)
    }

    fn write_interrupt(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        Ok(self.handle.write_interrupt(endpoint, data, timeout)?)
    }

    fn read_manufacturer_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc
            .manufacturer_string_index
            .ok_or_else(|| anyhow::anyhow!("No manufacturer string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }

    fn read_product_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc
            .product_string_index
            .ok_or_else(|| anyhow::anyhow!("No product string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }

    fn read_serial_number_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        let idx = desc
            .serial_number_string_index
            .ok_or_else(|| anyhow::anyhow!("No serial number string index"))?;
        Ok(self.handle.read_string_descriptor_ascii(idx)?)
    }
}

pub type MockTransferCallback =
    Arc<dyn Fn(String, Vec<u8>) -> anyhow::Result<Vec<u8>> + Send + Sync>;

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
    pub manufacturer: String,
    pub product: String,
    pub serial_number: String,
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
            manufacturer: self.manufacturer.clone(),
            product: self.product.clone(),
            serial_number: self.serial_number.clone(),
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

    fn read_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!(
                "control_read:{}:{}:{}:{}",
                request_type, request, value, index
            );
            let res = handler(action, vec![])?;
            let len = res.len().min(buf.len());
            buf[..len].copy_from_slice(&res[..len]);
            Ok(len)
        } else {
            Ok(0)
        }
    }

    fn write_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!(
                "control_write:{}:{}:{}:{}",
                request_type, request, value, index
            );
            let res = handler(action, data.to_vec())?;
            Ok(res.len())
        } else {
            Ok(0)
        }
    }

    fn read_bulk(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
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

    fn write_bulk(
        &mut self,
        endpoint: u8,
        data: &[u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
        if let Some(ref handler) = self.transfer_handler {
            let action = format!("bulk_write:{}", endpoint);
            let res = handler(action, data.to_vec())?;
            Ok(res.len())
        } else {
            Ok(0)
        }
    }

    fn read_interrupt(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
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

    fn write_interrupt(
        &mut self,
        endpoint: u8,
        data: &[u8],
        _timeout: Duration,
    ) -> anyhow::Result<usize> {
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

pub enum AnyUsbDevice {
    Physical(PhysicalUsbDevice),
    Mock(MockUsbDevice),
}

pub enum AnyUsbDeviceHandle {
    Physical(PhysicalUsbDeviceHandle),
    Mock(MockUsbDeviceHandle),
}

impl UsbDevice for AnyUsbDevice {
    type Handle = AnyUsbDeviceHandle;

    fn bus_number(&self) -> u8 {
        match self {
            Self::Physical(d) => d.bus_number(),
            Self::Mock(d) => d.bus_number(),
        }
    }

    fn address(&self) -> u8 {
        match self {
            Self::Physical(d) => d.address(),
            Self::Mock(d) => d.address(),
        }
    }

    fn speed(&self) -> UsbSpeed {
        match self {
            Self::Physical(d) => d.speed(),
            Self::Mock(d) => d.speed(),
        }
    }

    fn device_descriptor(&self) -> anyhow::Result<UsbDeviceDescriptor> {
        match self {
            Self::Physical(d) => d.device_descriptor(),
            Self::Mock(d) => d.device_descriptor(),
        }
    }

    fn config_descriptor(&self, index: u8) -> anyhow::Result<UsbConfigDescriptor> {
        match self {
            Self::Physical(d) => d.config_descriptor(index),
            Self::Mock(d) => d.config_descriptor(index),
        }
    }

    fn open(&self) -> anyhow::Result<Self::Handle> {
        match self {
            Self::Physical(d) => Ok(AnyUsbDeviceHandle::Physical(d.open()?)),
            Self::Mock(d) => Ok(AnyUsbDeviceHandle::Mock(d.open()?)),
        }
    }
}

impl UsbDeviceHandle for AnyUsbDeviceHandle {
    fn active_configuration(&self) -> anyhow::Result<u8> {
        match self {
            Self::Physical(h) => h.active_configuration(),
            Self::Mock(h) => h.active_configuration(),
        }
    }

    fn set_active_configuration(&mut self, config: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.set_active_configuration(config),
            Self::Mock(h) => h.set_active_configuration(config),
        }
    }

    fn claim_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.claim_interface(interface),
            Self::Mock(h) => h.claim_interface(interface),
        }
    }

    fn release_interface(&mut self, interface: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.release_interface(interface),
            Self::Mock(h) => h.release_interface(interface),
        }
    }

    fn set_alternate_setting(&mut self, interface: u8, setting: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.set_alternate_setting(interface, setting),
            Self::Mock(h) => h.set_alternate_setting(interface, setting),
        }
    }

    fn detach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.detach_kernel_driver(interface),
            Self::Mock(h) => h.detach_kernel_driver(interface),
        }
    }

    fn attach_kernel_driver(&mut self, interface: u8) -> anyhow::Result<()> {
        match self {
            Self::Physical(h) => h.attach_kernel_driver(interface),
            Self::Mock(h) => h.attach_kernel_driver(interface),
        }
    }

    fn kernel_driver_active(&self, interface: u8) -> anyhow::Result<bool> {
        match self {
            Self::Physical(h) => h.kernel_driver_active(interface),
            Self::Mock(h) => h.kernel_driver_active(interface),
        }
    }

    fn read_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => h.read_control(request_type, request, value, index, buf, timeout),
            Self::Mock(h) => h.read_control(request_type, request, value, index, buf, timeout),
        }
    }

    fn write_control(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => {
                h.write_control(request_type, request, value, index, data, timeout)
            }
            Self::Mock(h) => h.write_control(request_type, request, value, index, data, timeout),
        }
    }

    fn read_bulk(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => h.read_bulk(endpoint, buf, timeout),
            Self::Mock(h) => h.read_bulk(endpoint, buf, timeout),
        }
    }

    fn write_bulk(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => h.write_bulk(endpoint, data, timeout),
            Self::Mock(h) => h.write_bulk(endpoint, data, timeout),
        }
    }

    fn read_interrupt(
        &mut self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => h.read_interrupt(endpoint, buf, timeout),
            Self::Mock(h) => h.read_interrupt(endpoint, buf, timeout),
        }
    }

    fn write_interrupt(
        &mut self,
        endpoint: u8,
        data: &[u8],
        timeout: Duration,
    ) -> anyhow::Result<usize> {
        match self {
            Self::Physical(h) => h.write_interrupt(endpoint, data, timeout),
            Self::Mock(h) => h.write_interrupt(endpoint, data, timeout),
        }
    }

    fn read_manufacturer_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        match self {
            Self::Physical(h) => h.read_manufacturer_string(desc),
            Self::Mock(h) => h.read_manufacturer_string(desc),
        }
    }

    fn read_product_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        match self {
            Self::Physical(h) => h.read_product_string(desc),
            Self::Mock(h) => h.read_product_string(desc),
        }
    }

    fn read_serial_number_string(&self, desc: &UsbDeviceDescriptor) -> anyhow::Result<String> {
        match self {
            Self::Physical(h) => h.read_serial_number_string(desc),
            Self::Mock(h) => h.read_serial_number_string(desc),
        }
    }
}

pub async fn run_mock_kernel_client(local_port: u16, busid: &str) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    // 1. Perform OP_REQ_DEVLIST
    let mut stream1 = TcpStream::connect(format!("127.0.0.1:{}", local_port)).await?;
    let mut devlist_req = Vec::new();
    devlist_req.extend_from_slice(&[
        0x01, 0x11, // version: 0x0111
        0x80, 0x05, // code: OP_REQ_DEVLIST (0x8005)
        0x00, 0x00, 0x00, 0x00, // status: 0
    ]);
    stream1.write_all(&devlist_req).await?;
    stream1.flush().await?;

    let mut header = [0u8; 8];
    stream1.read_exact(&mut header).await?;
    assert_eq!(&header[0..2], &[0x01, 0x11]);
    assert_eq!(&header[2..4], &[0x00, 0x05]); // OP_REP_DEVLIST
    assert_eq!(&header[4..8], &[0x00, 0x00, 0x00, 0x00]);

    let mut ndev_buf = [0u8; 4];
    stream1.read_exact(&mut ndev_buf).await?;
    let ndev = u32::from_be_bytes(ndev_buf);
    assert_eq!(ndev, 1, "Expected 1 mock device");

    let mut udev_buf = [0u8; 312];
    stream1.read_exact(&mut udev_buf).await?;
    let dev = protocol::UsbipUsbDevice::from_bytes(&udev_buf);
    assert_eq!(dev.id_vendor, 0x1d6b);
    assert_eq!(dev.id_product, 0x0104);

    for _ in 0..dev.b_num_interfaces {
        let mut intf_buf = [0u8; 4];
        stream1.read_exact(&mut intf_buf).await?;
    }
    drop(stream1);

    // 2. Perform OP_REQ_IMPORT
    let mut stream2 = TcpStream::connect(format!("127.0.0.1:{}", local_port)).await?;
    let mut import_req = Vec::new();
    import_req.extend_from_slice(&[
        0x01, 0x11, // version
        0x80, 0x03, // OP_REQ_IMPORT
        0x00, 0x00, 0x00, 0x00, // status
    ]);
    import_req.extend_from_slice(&protocol::pad_string(busid, 32));
    stream2.write_all(&import_req).await?;
    stream2.flush().await?;

    let mut import_header = [0u8; 8];
    stream2.read_exact(&mut import_header).await?;
    assert_eq!(&import_header[0..2], &[0x01, 0x11]);
    assert_eq!(&import_header[2..4], &[0x00, 0x03]); // OP_REP_IMPORT
    let import_status = u32::from_be_bytes([
        import_header[4],
        import_header[5],
        import_header[6],
        import_header[7],
    ]);
    assert_eq!(import_status, 0);

    let mut udev_buf = [0u8; 312];
    stream2.read_exact(&mut udev_buf).await?;
    let dev = protocol::UsbipUsbDevice::from_bytes(&udev_buf);
    assert_eq!(dev.id_vendor, 0x1d6b);
    assert_eq!(dev.id_product, 0x0104);

    // 3. Send USBIP_CMD_SUBMIT to fetch the device descriptor
    let seqnum = 42u32;
    let devid = 0u32;
    let direction = 1u32; // IN
    let ep = 0u32;
    let transfer_flags = 0u32;
    let transfer_buffer_length = 18i32;
    let setup = [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 18, 0];

    let mut cmd = Vec::new();
    cmd.extend_from_slice(&0x0001u32.to_be_bytes()); // command: USBIP_CMD_SUBMIT (1)
    cmd.extend_from_slice(&seqnum.to_be_bytes());
    cmd.extend_from_slice(&devid.to_be_bytes());
    cmd.extend_from_slice(&direction.to_be_bytes());
    cmd.extend_from_slice(&ep.to_be_bytes());
    cmd.extend_from_slice(&transfer_flags.to_be_bytes());
    cmd.extend_from_slice(&transfer_buffer_length.to_be_bytes());
    cmd.extend_from_slice(&0i32.to_be_bytes()); // start_frame
    cmd.extend_from_slice(&0i32.to_be_bytes()); // number_of_packets
    cmd.extend_from_slice(&0i32.to_be_bytes()); // interval
    cmd.extend_from_slice(&setup);

    stream2.write_all(&cmd).await?;
    stream2.flush().await?;

    // 4. Read USBIP_RET_SUBMIT response
    let mut ret_header = [0u8; 48];
    stream2.read_exact(&mut ret_header).await?;

    let command = u32::from_be_bytes([ret_header[0], ret_header[1], ret_header[2], ret_header[3]]);
    assert_eq!(command, 0x0003); // USBIP_RET_SUBMIT (3)
    let resp_seqnum =
        u32::from_be_bytes([ret_header[4], ret_header[5], ret_header[6], ret_header[7]]);
    assert_eq!(resp_seqnum, seqnum);
    let status = i32::from_be_bytes([
        ret_header[20],
        ret_header[21],
        ret_header[22],
        ret_header[23],
    ]);
    assert_eq!(status, 0);
    let actual_len = i32::from_be_bytes([
        ret_header[24],
        ret_header[25],
        ret_header[26],
        ret_header[27],
    ]);
    assert_eq!(actual_len, 18);

    let mut payload = vec![0u8; 18];
    stream2.read_exact(&mut payload).await?;

    // Assert that the returned packet descriptors match the mock device setup
    assert_eq!(payload[8], 0x6b);
    assert_eq!(payload[9], 0x1d);
    assert_eq!(payload[10], 0x04);
    assert_eq!(payload[11], 0x01);

    drop(stream2);
    Ok(())
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
            manufacturer: "Mock Manufacturer".to_string(),
            product: "Mock Product".to_string(),
            serial_number: "Mock Serial".to_string(),
        };

        assert_eq!(dev.bus_number(), 1);
        assert_eq!(dev.address(), 2);
        assert_eq!(dev.speed(), UsbSpeed::High);
        assert_eq!(dev.device_descriptor().unwrap(), desc);

        let handle = dev.open().unwrap();
        assert_eq!(handle.active_configuration().unwrap(), 1);
        assert_eq!(
            handle.read_manufacturer_string(&desc).unwrap(),
            "Mock Manufacturer"
        );
        assert_eq!(handle.read_product_string(&desc).unwrap(), "Mock Product");
        assert_eq!(
            handle.read_serial_number_string(&desc).unwrap(),
            "Mock Serial"
        );
    }
}
