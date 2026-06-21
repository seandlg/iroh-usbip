use crate::{UsbSpeed, UsbDevice};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipHeaderBasic {
    pub command: u32,
    pub seqnum: u32,
    pub devid: u32,
    pub direction: u32,
    pub ep: u32,
}

impl UsbipHeaderBasic {
    pub fn to_bytes(&self) -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0..4].copy_from_slice(&self.command.to_be_bytes());
        buf[4..8].copy_from_slice(&self.seqnum.to_be_bytes());
        buf[8..12].copy_from_slice(&self.devid.to_be_bytes());
        buf[12..16].copy_from_slice(&self.direction.to_be_bytes());
        buf[16..20].copy_from_slice(&self.ep.to_be_bytes());
        buf
    }

    pub fn from_bytes(bytes: [u8; 20]) -> Self {
        Self {
            command: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            seqnum: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            devid: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            direction: u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            ep: u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipHeaderCmdSubmit {
    pub transfer_flags: u32,
    pub transfer_buffer_length: i32,
    pub start_frame: i32,
    pub number_of_packets: i32,
    pub interval: i32,
    pub setup: [u8; 8],
}

impl UsbipHeaderCmdSubmit {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..4].copy_from_slice(&self.transfer_flags.to_be_bytes());
        buf[4..8].copy_from_slice(&self.transfer_buffer_length.to_be_bytes());
        buf[8..12].copy_from_slice(&self.start_frame.to_be_bytes());
        buf[12..16].copy_from_slice(&self.number_of_packets.to_be_bytes());
        buf[16..20].copy_from_slice(&self.interval.to_be_bytes());
        buf[20..28].copy_from_slice(&self.setup);
        buf
    }

    pub fn from_bytes(bytes: [u8; 28]) -> Self {
        let mut setup = [0u8; 8];
        setup.copy_from_slice(&bytes[20..28]);
        Self {
            transfer_flags: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            transfer_buffer_length: i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            start_frame: i32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            number_of_packets: i32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            interval: i32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            setup,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipHeaderRetSubmit {
    pub status: i32,
    pub actual_length: i32,
    pub start_frame: i32,
    pub number_of_packets: i32,
    pub error_count: i32,
}

impl UsbipHeaderRetSubmit {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..4].copy_from_slice(&self.status.to_be_bytes());
        buf[4..8].copy_from_slice(&self.actual_length.to_be_bytes());
        buf[8..12].copy_from_slice(&self.start_frame.to_be_bytes());
        buf[12..16].copy_from_slice(&self.number_of_packets.to_be_bytes());
        buf[16..20].copy_from_slice(&self.error_count.to_be_bytes());
        // Remaining 8 bytes (20..28) are zeroed setup padding
        buf
    }

    pub fn from_bytes(bytes: [u8; 28]) -> Self {
        Self {
            status: i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            actual_length: i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            start_frame: i32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            number_of_packets: i32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
            error_count: i32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipHeaderCmdUnlink {
    pub seqnum: u32,
}

impl UsbipHeaderCmdUnlink {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..4].copy_from_slice(&self.seqnum.to_be_bytes());
        // Remaining 24 bytes (4..28) are zeroed padding
        buf
    }

    pub fn from_bytes(bytes: [u8; 28]) -> Self {
        Self {
            seqnum: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipHeaderRetUnlink {
    pub status: i32,
}

impl UsbipHeaderRetUnlink {
    pub fn to_bytes(&self) -> [u8; 28] {
        let mut buf = [0u8; 28];
        buf[0..4].copy_from_slice(&self.status.to_be_bytes());
        // Remaining 24 bytes (4..28) are zeroed padding
        buf
    }

    pub fn from_bytes(bytes: [u8; 28]) -> Self {
        Self {
            status: i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbipIsoPacketDescriptor {
    pub offset: u32,
    pub length: u32,
    pub actual_length: u32,
    pub status: u32,
}

impl UsbipIsoPacketDescriptor {
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.offset.to_be_bytes());
        buf[4..8].copy_from_slice(&self.length.to_be_bytes());
        buf[8..12].copy_from_slice(&self.actual_length.to_be_bytes());
        buf[12..16].copy_from_slice(&self.status.to_be_bytes());
        buf
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self {
            offset: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            length: u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            actual_length: u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            status: u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        }
    }
}

impl UsbipUsbDevice {
    pub fn new<D: UsbDevice + ?Sized>(dev: &D) -> anyhow::Result<Self> {
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

        let path_buf = crate::HostDeviceRegistry::<D>::sysfs_path(busnum, addr);
        let path_str = path_buf.to_string_lossy();
        let path_bytes = pad_string(&path_str, 256);
        let mut path = [0u8; 256];
        path.copy_from_slice(&path_bytes);

        let busid_str = crate::HostDeviceRegistry::<D>::bus_id(busnum, addr);
        let busid_bytes = pad_string(&busid_str, 32);
        let mut busid = [0u8; 32];
        busid.copy_from_slice(&busid_bytes);

        Ok(Self {
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
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsbipDeviceDetail {
    pub device: UsbipUsbDevice,
    pub interfaces: Vec<UsbipUsbInterface>,
}

impl UsbipDeviceDetail {
    pub fn new<D: UsbDevice + ?Sized>(dev: &D) -> anyhow::Result<Self> {
        let udev = UsbipUsbDevice::new(dev)?;
        let mut interfaces = Vec::new();
        if let Ok(config) = dev.config_descriptor(0) {
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
                interfaces.push(UsbipUsbInterface {
                    b_interface_class: setting.class_code,
                    b_interface_subclass: setting.sub_class_code,
                    b_interface_protocol: setting.protocol_code,
                    padding: 0,
                });
            }
        }
        Ok(Self {
            device: udev,
            interfaces,
        })
    }
}

#[derive(Debug)]
pub enum UsbipRequest {
    Devlist,
    Import {
        busid: String,
    },
    Submit {
        basic: UsbipHeaderBasic,
        submit: UsbipHeaderCmdSubmit,
        data: Vec<u8>,
        iso_descriptors: Vec<UsbipIsoPacketDescriptor>,
    },
    Unlink {
        basic: UsbipHeaderBasic,
        unlink: UsbipHeaderCmdUnlink,
    },
}

#[derive(Debug)]
pub enum UsbipResponse {
    Devlist {
        devices: Vec<UsbipDeviceDetail>,
    },
    Import {
        status: u32,
        device: Option<UsbipDeviceDetail>,
    },
    Submit {
        basic: UsbipHeaderBasic,
        submit: UsbipHeaderRetSubmit,
        data: Vec<u8>,
        iso_descriptors: Vec<UsbipIsoPacketDescriptor>,
    },
    Unlink {
        basic: UsbipHeaderBasic,
        unlink: UsbipHeaderRetUnlink,
    },
}

pub struct UsbipStream<S> {
    stream: S,
}

impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> UsbipStream<S> {
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    pub async fn read_handshake_request(&mut self) -> anyhow::Result<Option<UsbipRequest>> {
        let mut header = [0u8; 8];
        if let Err(e) = self.stream.read_exact(&mut header).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(e.into());
        }

        let common = OpCommon::from_bytes(header);
        if common.version != USBIP_VERSION {
            anyhow::bail!("Unsupported USBIP version: {:04x}", common.version);
        }

        match common.code {
            OP_REQ_DEVLIST => Ok(Some(UsbipRequest::Devlist)),
            OP_REQ_IMPORT => {
                let mut busid_buf = [0u8; 32];
                self.stream.read_exact(&mut busid_buf).await?;
                let import_req = OpImportRequest::from_bytes(busid_buf);
                let busid = std::str::from_utf8(&import_req.busid)?
                    .trim_end_matches('\0')
                    .to_string();
                Ok(Some(UsbipRequest::Import { busid }))
            }
            _ => anyhow::bail!("Unknown USBIP command code: {:04x}", common.code),
        }
    }

    pub async fn write_handshake_response(&mut self, response: UsbipResponse) -> anyhow::Result<()> {
        match response {
            UsbipResponse::Devlist { devices } => {
                let mut buf = Vec::new();
                let rep_header = OpCommon {
                    version: USBIP_VERSION,
                    code: OP_REP_DEVLIST,
                    status: ST_OK,
                };
                buf.extend_from_slice(&rep_header.to_bytes());

                let rep_devlist = OpDevlistReply {
                    ndev: devices.len() as u32,
                };
                buf.extend_from_slice(&rep_devlist.to_bytes());

                for dev in &devices {
                    buf.extend_from_slice(&dev.device.to_bytes());
                    for interface in &dev.interfaces {
                        buf.extend_from_slice(&interface.to_bytes());
                    }
                }

                self.stream.write_all(&buf).await?;
                self.stream.flush().await?;
            }
            UsbipResponse::Import { status, device } => {
                let mut buf = Vec::new();
                let rep_header = OpCommon {
                    version: USBIP_VERSION,
                    code: OP_REP_IMPORT,
                    status,
                };
                buf.extend_from_slice(&rep_header.to_bytes());

                if let Some(dev) = device {
                    buf.extend_from_slice(&dev.device.to_bytes());
                }

                self.stream.write_all(&buf).await?;
                self.stream.flush().await?;
            }
            _ => anyhow::bail!("Invalid handshake response"),
        }
        Ok(())
    }

    pub async fn read_transfer_request(&mut self) -> anyhow::Result<Option<UsbipRequest>> {
        let mut cmd_buf = [0u8; 48];
        if let Err(e) = self.stream.read_exact(&mut cmd_buf).await {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(e.into());
        }

        let mut basic_bytes = [0u8; 20];
        basic_bytes.copy_from_slice(&cmd_buf[0..20]);
        let basic = UsbipHeaderBasic::from_bytes(basic_bytes);

        let mut payload_bytes = [0u8; 28];
        payload_bytes.copy_from_slice(&cmd_buf[20..48]);

        match basic.command {
            USBIP_CMD_SUBMIT => {
                let cmd_submit = UsbipHeaderCmdSubmit::from_bytes(payload_bytes);

                // Read OUT data if direction is OUT (0) and transfer_buffer_length > 0
                let mut data = vec![0u8; cmd_submit.transfer_buffer_length.max(0) as usize];
                if basic.direction == 0 && cmd_submit.transfer_buffer_length > 0 {
                    self.stream.read_exact(&mut data).await?;
                }

                // Read isochronous packet descriptors if number_of_packets > 0
                let mut iso_descriptors = Vec::new();
                if cmd_submit.number_of_packets > 0 {
                    let total_desc_bytes = (cmd_submit.number_of_packets as usize) * 16;
                    let mut desc_buf = vec![0u8; total_desc_bytes];
                    self.stream.read_exact(&mut desc_buf).await?;
                    for chunk in desc_buf.chunks_exact(16) {
                        let mut arr = [0u8; 16];
                        arr.copy_from_slice(chunk);
                        iso_descriptors.push(UsbipIsoPacketDescriptor::from_bytes(arr));
                    }
                }

                Ok(Some(UsbipRequest::Submit {
                    basic,
                    submit: cmd_submit,
                    data,
                    iso_descriptors,
                }))
            }
            USBIP_CMD_UNLINK => {
                let cmd_unlink = UsbipHeaderCmdUnlink::from_bytes(payload_bytes);
                Ok(Some(UsbipRequest::Unlink {
                    basic,
                    unlink: cmd_unlink,
                }))
            }
            _ => anyhow::bail!("Unknown transfer command: {:08x}", basic.command),
        }
    }

    pub async fn write_transfer_response(&mut self, response: UsbipResponse) -> anyhow::Result<()> {
        match response {
            UsbipResponse::Submit {
                basic,
                submit,
                data,
                iso_descriptors,
            } => {
                let mut resp = [0u8; 48];
                resp[0..20].copy_from_slice(&basic.to_bytes());
                resp[20..48].copy_from_slice(&submit.to_bytes());

                self.stream.write_all(&resp).await?;
                if basic.direction == 1 {
                    self.stream.write_all(&data).await?;
                }

                // Write back dummy descriptors if number_of_packets > 0 and IN transfer
                if basic.direction == 1 && submit.number_of_packets > 0 {
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
                    self.stream.write_all(&dummy_desc_bytes).await?;
                }

                self.stream.flush().await?;
            }
            UsbipResponse::Unlink { basic, unlink } => {
                let mut resp = [0u8; 48];
                resp[0..20].copy_from_slice(&basic.to_bytes());
                resp[20..48].copy_from_slice(&unlink.to_bytes());

                self.stream.write_all(&resp).await?;
                self.stream.flush().await?;
            }
            _ => anyhow::bail!("Invalid transfer response"),
        }
        Ok(())
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_usbip_iso_packet_descriptor_serialization() {
        let desc = UsbipIsoPacketDescriptor {
            offset: 1024,
            length: 512,
            actual_length: 256,
            status: 0,
        };
        let bytes = desc.to_bytes();
        let expected = [
            0x00, 0x00, 0x04, 0x00, // offset: 1024
            0x00, 0x00, 0x02, 0x00, // length: 512
            0x00, 0x00, 0x01, 0x00, // actual_length: 256
            0x00, 0x00, 0x00, 0x00, // status: 0
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipIsoPacketDescriptor::from_bytes(bytes);
        assert_eq!(deserialized, desc);
    }

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

    #[test]
    fn test_usbip_header_basic_serialization() {
        let basic = UsbipHeaderBasic {
            command: 0x0001,
            seqnum: 42,
            devid: 100,
            direction: 1,
            ep: 3,
        };
        let bytes = basic.to_bytes();
        let expected = [
            0x00, 0x00, 0x00, 0x01, // command
            0x00, 0x00, 0x00, 0x2a, // seqnum (42)
            0x00, 0x00, 0x00, 0x64, // devid (100)
            0x00, 0x00, 0x00, 0x01, // direction
            0x00, 0x00, 0x00, 0x03, // ep
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipHeaderBasic::from_bytes(bytes);
        assert_eq!(deserialized, basic);
    }

    #[test]
    fn test_usbip_header_cmd_submit_serialization() {
        let cmd_submit = UsbipHeaderCmdSubmit {
            transfer_flags: 1,
            transfer_buffer_length: 512,
            start_frame: 2,
            number_of_packets: 3,
            interval: 4,
            setup: [1, 2, 3, 4, 5, 6, 7, 8],
        };
        let bytes = cmd_submit.to_bytes();
        let expected = [
            0x00, 0x00, 0x00, 0x01, // transfer_flags
            0x00, 0x00, 0x02, 0x00, // transfer_buffer_length (512)
            0x00, 0x00, 0x00, 0x02, // start_frame
            0x00, 0x00, 0x00, 0x03, // number_of_packets
            0x00, 0x00, 0x00, 0x04, // interval
            1, 2, 3, 4, 5, 6, 7, 8, // setup
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipHeaderCmdSubmit::from_bytes(bytes);
        assert_eq!(deserialized, cmd_submit);
    }

    #[test]
    fn test_usbip_header_ret_submit_serialization() {
        let ret_submit = UsbipHeaderRetSubmit {
            status: -32,
            actual_length: 128,
            start_frame: 0,
            number_of_packets: 0,
            error_count: 0,
        };
        let bytes = ret_submit.to_bytes();
        let expected = [
            0xff, 0xff, 0xff, 0xe0, // status (-32)
            0x00, 0x00, 0x00, 0x80, // actual_length (128)
            0x00, 0x00, 0x00, 0x00, // start_frame (0)
            0x00, 0x00, 0x00, 0x00, // number_of_packets (0)
            0x00, 0x00, 0x00, 0x00, // error_count (0)
            0, 0, 0, 0, 0, 0, 0, 0, // setup padding (8 bytes)
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipHeaderRetSubmit::from_bytes(bytes);
        assert_eq!(deserialized, ret_submit);
    }

    #[test]
    fn test_usbip_header_cmd_unlink_serialization() {
        let cmd_unlink = UsbipHeaderCmdUnlink { seqnum: 99 };
        let bytes = cmd_unlink.to_bytes();
        let expected = [
            0x00, 0x00, 0x00, 0x63, // seqnum (99)
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, // 24 bytes of padding
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipHeaderCmdUnlink::from_bytes(bytes);
        assert_eq!(deserialized, cmd_unlink);
    }

    #[test]
    fn test_usbip_header_ret_unlink_serialization() {
        let ret_unlink = UsbipHeaderRetUnlink { status: -104 };
        let bytes = ret_unlink.to_bytes();
        let expected = [
            0xff, 0xff, 0xff, 0x98, // status (-104)
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, // 24 bytes of padding
        ];
        assert_eq!(bytes, expected);

        let deserialized = UsbipHeaderRetUnlink::from_bytes(bytes);
        assert_eq!(deserialized, ret_unlink);
    }

    #[tokio::test]
    async fn test_usbip_stream_handshake() {
        let (client, host) = tokio::io::duplex(1024);
        let mut client_stream = UsbipStream::new(client);
        let mut host_stream = UsbipStream::new(host);

        // Client writes OP_REQ_DEVLIST
        let req_bytes = [
            0x01, 0x11, // version: 0x0111
            0x80, 0x05, // code: OP_REQ_DEVLIST (0x8005)
            0x00, 0x00, 0x00, 0x00, // status: 0
        ];
        client_stream.stream.write_all(&req_bytes).await.unwrap();

        // Host reads the request
        let req = host_stream.read_handshake_request().await.unwrap();
        assert!(matches!(req, Some(UsbipRequest::Devlist)));

        // Host writes empty devlist response
        host_stream.write_handshake_response(UsbipResponse::Devlist { devices: vec![] }).await.unwrap();

        // Client reads response header
        let mut resp_header = [0u8; 8];
        client_stream.stream.read_exact(&mut resp_header).await.unwrap();
        assert_eq!(&resp_header[0..2], &[0x01, 0x11]);
        assert_eq!(&resp_header[2..4], &[0x00, 0x05]); // OP_REP_DEVLIST

        let mut ndev_bytes = [0u8; 4];
        client_stream.stream.read_exact(&mut ndev_bytes).await.unwrap();
        let ndev = u32::from_be_bytes(ndev_bytes);
        assert_eq!(ndev, 0);
    }
}

