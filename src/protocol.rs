use crate::UsbSpeed;

pub const OP_REQ_DEVLIST: u16 = 0x8005;
pub const OP_REP_DEVLIST: u16 = 0x0005;
pub const OP_REQ_IMPORT: u16 = 0x8003;
pub const OP_REP_IMPORT: u16 = 0x0003;

pub const USBIP_VERSION: u16 = 0x0111;

pub const USBIP_CMD_SUBMIT: u32 = 0x0001;
pub const USBIP_CMD_UNLINK: u32 = 0x0002;
pub const USBIP_RET_SUBMIT: u32 = 0x0003;
pub const USBIP_RET_UNLINK: u32 = 0x0004;

pub const SYSFS_PATH_MAX: usize = 256;
pub const SYSFS_BUS_ID_SIZE: usize = 32;

pub fn map_speed(speed: UsbSpeed) -> u32 {
    match speed {
        UsbSpeed::Low => 1,
        UsbSpeed::Full => 2,
        UsbSpeed::High => 3,
        UsbSpeed::Super => 5,
        UsbSpeed::SuperPlus => 6,
        UsbSpeed::Unknown => 0,
    }
}

pub fn pad_string(s: &str, len: usize) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    if v.len() > len {
        v.truncate(len);
    } else {
        v.resize(len, 0);
    }
    v
}

#[derive(Debug, Clone)]
pub struct UsbipUsbDevice {
    pub path: [u8; 256],
    pub busid: [u8; 32],
    pub busnum: u32,
    pub devnum: u32,
    pub speed: u32,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub b_device_class: u8,
    pub b_device_subclass: u8,
    pub b_device_protocol: u8,
    pub b_configuration_value: u8,
    pub b_num_configurations: u8,
    pub b_num_interfaces: u8,
}

impl UsbipUsbDevice {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(312);
        buf.extend_from_slice(&self.path);
        buf.extend_from_slice(&self.busid);
        buf.extend_from_slice(&self.busnum.to_be_bytes());
        buf.extend_from_slice(&self.devnum.to_be_bytes());
        buf.extend_from_slice(&self.speed.to_be_bytes());
        buf.extend_from_slice(&self.id_vendor.to_be_bytes());
        buf.extend_from_slice(&self.id_product.to_be_bytes());
        buf.extend_from_slice(&self.bcd_device.to_be_bytes());
        buf.push(self.b_device_class);
        buf.push(self.b_device_subclass);
        buf.push(self.b_device_protocol);
        buf.push(self.b_configuration_value);
        buf.push(self.b_num_configurations);
        buf.push(self.b_num_interfaces);
        buf
    }
}

#[derive(Debug, Clone)]
pub struct UsbipUsbInterface {
    pub b_interface_class: u8,
    pub b_interface_subclass: u8,
    pub b_interface_protocol: u8,
    pub padding: u8,
}

impl UsbipUsbInterface {
    pub fn to_bytes(&self) -> [u8; 4] {
        [
            self.b_interface_class,
            self.b_interface_subclass,
            self.b_interface_protocol,
            self.padding,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpCommon {
    pub version: u16,
    pub code: u16,
    pub status: u32,
}

impl OpCommon {
    pub fn to_bytes(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0..2].copy_from_slice(&self.version.to_be_bytes());
        buf[2..4].copy_from_slice(&self.code.to_be_bytes());
        buf[4..8].copy_from_slice(&self.status.to_be_bytes());
        buf
    }

    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self {
            version: u16::from_be_bytes([bytes[0], bytes[1]]),
            code: u16::from_be_bytes([bytes[2], bytes[3]]),
            status: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpDevlistReply {
    pub ndev: u32,
}

impl OpDevlistReply {
    pub fn to_bytes(&self) -> [u8; 4] {
        self.ndev.to_be_bytes()
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            ndev: u32::from_be_bytes(bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpImportRequest {
    pub busid: [u8; 32],
}

impl OpImportRequest {
    pub fn to_bytes(&self) -> [u8; 32] {
        self.busid
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { busid: bytes }
    }
}

pub const ST_OK: u32 = 0x00;
pub const ST_NA: u32 = 0x01;
pub const ST_DEV_BUSY: u32 = 0x02;

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_op_common_serialization() {
        let common = OpCommon {
            version: 0x0111,
            code: 0x8005,
            status: 42,
        };
        let bytes = common.to_bytes();
        assert_eq!(bytes, [0x01, 0x11, 0x80, 0x05, 0x00, 0x00, 0x00, 0x2a]);
        
        let deserialized = OpCommon::from_bytes(bytes);
        assert_eq!(deserialized, common);
    }

    #[test]
    fn test_op_devlist_reply_serialization() {
        let reply = OpDevlistReply { ndev: 5 };
        let bytes = reply.to_bytes();
        assert_eq!(bytes, [0x00, 0x00, 0x00, 0x05]);

        let deserialized = OpDevlistReply::from_bytes(bytes);
        assert_eq!(deserialized, reply);
    }

    #[test]
    fn test_op_import_request_serialization() {
        let mut busid = [0u8; 32];
        busid[0..3].copy_from_slice(b"1-2");
        let request = OpImportRequest { busid };
        let bytes = request.to_bytes();
        assert_eq!(bytes, busid);

        let deserialized = OpImportRequest::from_bytes(bytes);
        assert_eq!(deserialized, request);
    }
}

