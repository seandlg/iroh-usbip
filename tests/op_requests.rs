mod common;

use common::{MockDeviceBuilder, TestContext};

#[tokio::test]
async fn test_op_req_devlist() -> anyhow::Result<()> {
    let dev = MockDeviceBuilder::new().build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_devlist().await?;
    let (ndev, udev_list) = ctx.read_op_rep_devlist().await?;

    assert_eq!(ndev, 1);
    assert_eq!(udev_list.len(), 1);

    let udev_buf = &udev_list[0];
    let vendor_id = u16::from_be_bytes([udev_buf[300], udev_buf[301]]);
    let product_id = u16::from_be_bytes([udev_buf[302], udev_buf[303]]);
    assert_eq!(vendor_id, 0x1234);
    assert_eq!(product_id, 0x5678);

    ctx.host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_op_req_import() -> anyhow::Result<()> {
    let dev = MockDeviceBuilder::new().build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, udev_buf) = ctx.read_op_rep_import().await?;

    assert_eq!(status, 0);
    assert_eq!(udev_buf.len(), 312);

    let vendor_id = u16::from_be_bytes([udev_buf[300], udev_buf[301]]);
    let product_id = u16::from_be_bytes([udev_buf[302], udev_buf[303]]);
    assert_eq!(vendor_id, 0x1234);
    assert_eq!(product_id, 0x5678);

    drop(ctx.client);
    ctx.host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_op_req_import_device_busy() -> anyhow::Result<()> {
    let dev = MockDeviceBuilder::new()
        .with_open_error("device is busy".to_string())
        .build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;

    assert_eq!(status, 2); // ST_DEV_BUSY = 0x02

    drop(ctx.client);
    ctx.host_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_op_req_import_device_not_available() -> anyhow::Result<()> {
    let dev = MockDeviceBuilder::new()
        .with_open_error("general permission/hardware error".to_string())
        .build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;

    assert_eq!(status, 1); // ST_NA = 0x01

    drop(ctx.client);
    ctx.host_handle.await??;
    Ok(())
}
