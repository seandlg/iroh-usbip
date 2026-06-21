mod common;

use common::{MockDeviceBuilder, TestContext};
use iroh_usbip::protocol::UsbipIsoPacketDescriptor;
use std::sync::Arc;

#[tokio::test]
async fn test_urb_transfer() -> anyhow::Result<()> {
    let callback = Arc::new(|action: String, _data: Vec<u8>| {
        if action == "control_read:128:6:256:0" {
            Ok(vec![0x12, 0x34, 0x56, 0x78])
        } else {
            Ok(vec![])
        }
    });

    let dev = MockDeviceBuilder::new()
        .with_transfer_handler(callback)
        .build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;
    assert_eq!(status, 0);

    ctx.send_usbip_cmd_submit(
        10,
        0x00010002,
        1,                                                // direction: IN
        0,                                                // ep: 0
        0,                                                // transfer_flags
        4,                                                // transfer_buffer_length
        [0x80, 0x06, 0x00, 0x01, 0x00, 0x00, 0x04, 0x00], // setup
        &[],
    )
    .await?;

    let (seqnum, status, actual_len, number_of_packets, payload, iso_descs) =
        ctx.read_usbip_ret_submit().await?;

    assert_eq!(seqnum, 10);
    assert_eq!(status, 0);
    assert_eq!(actual_len, 4);
    assert_eq!(number_of_packets, 0);
    assert_eq!(payload, vec![0x12, 0x34, 0x56, 0x78]);
    assert!(iso_descs.is_empty());

    drop(ctx.client);
    let _ = ctx.host_handle.await;
    Ok(())
}

#[tokio::test]
async fn test_urb_unlink() -> anyhow::Result<()> {
    let dev = MockDeviceBuilder::new().build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;
    assert_eq!(status, 0);

    ctx.send_usbip_cmd_unlink(
        20, 0x00010002, 0,  // ep: 0
        10, // unlink_seqnum
    )
    .await?;

    let (seqnum, status) = ctx.read_usbip_ret_unlink().await?;
    assert_eq!(seqnum, 20);
    assert_eq!(status, -104); // -ECONNRESET

    drop(ctx.client);
    let _ = ctx.host_handle.await;
    Ok(())
}

#[tokio::test]
async fn test_isochronous_transfer_dummy_handling() -> anyhow::Result<()> {
    let callback = Arc::new(|action: String, _data: Vec<u8>| {
        if action == "bulk_read:129" {
            Ok(vec![0xAA; 64])
        } else {
            Ok(vec![])
        }
    });

    let dev = MockDeviceBuilder::new()
        .with_transfer_handler(callback)
        .build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;
    assert_eq!(status, 0);

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

    ctx.send_usbip_cmd_submit(
        10,
        0x00010002,
        1, // direction: IN
        1, // ep: 1
        0,
        64,
        [0; 8],
        &[desc1, desc2],
    )
    .await?;

    let (seqnum, status, actual_len, number_of_packets, payload, returned_descs) =
        ctx.read_usbip_ret_submit().await?;

    assert_eq!(seqnum, 10);
    assert_eq!(status, 0);
    assert_eq!(actual_len, 64);
    assert_eq!(number_of_packets, 2);
    assert_eq!(payload, vec![0xAA; 64]);
    assert_eq!(returned_descs.len(), 2);

    assert_eq!(returned_descs[0].offset, 0);
    assert_eq!(returned_descs[0].length, 32);
    assert_eq!(returned_descs[0].actual_length, 32);
    assert_eq!(returned_descs[0].status, 0);

    assert_eq!(returned_descs[1].offset, 32);
    assert_eq!(returned_descs[1].length, 32);
    assert_eq!(returned_descs[1].actual_length, 32);
    assert_eq!(returned_descs[1].status, 0);

    // Verify stream synchronization by sending a subsequent unlink command
    ctx.send_usbip_cmd_unlink(
        20, 0x00010002, 0,  // ep: 0
        10, // unlink_seqnum
    )
    .await?;

    let (unlink_seqnum, unlink_status) = ctx.read_usbip_ret_unlink().await?;
    assert_eq!(unlink_seqnum, 20);
    assert_eq!(unlink_status, -104);

    drop(ctx.client);
    let _ = ctx.host_handle.await;
    Ok(())
}
