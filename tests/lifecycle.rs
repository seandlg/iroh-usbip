mod common;

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::{HashMap, HashSet};
use common::{TestContext, MockDeviceBuilder};
use iroh_usbip::{UsbConfigDescriptor, UsbInterfaceDescriptor, UsbInterfaceSettingDescriptor};

#[tokio::test]
async fn test_disconnection_teardown() -> anyhow::Result<()> {
    let dropped_flag = Arc::new(AtomicBool::new(false));
    let dev = MockDeviceBuilder::new()
        .with_dropped(dropped_flag.clone())
        .build();
    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;
    assert_eq!(status, 0);

    drop(ctx.client);
    ctx.host_handle.await??;

    assert!(dropped_flag.load(Ordering::SeqCst));

    Ok(())
}

#[tokio::test]
async fn test_driver_detachment_lifecycle() -> anyhow::Result<()> {
    let kernel_drivers = Arc::new(Mutex::new(HashMap::new()));
    let claimed_interfaces = Arc::new(Mutex::new(HashSet::new()));

    let config = UsbConfigDescriptor {
        num_interfaces: 2,
        configuration_value: 1,
        max_power: 500,
        self_powered: true,
        remote_wakeup: false,
        interfaces: vec![
            UsbInterfaceDescriptor {
                interface_number: 0,
                settings: vec![UsbInterfaceSettingDescriptor {
                    setting_number: 0,
                    class_code: 0,
                    sub_class_code: 0,
                    protocol_code: 0,
                    endpoints: vec![],
                }],
            },
            UsbInterfaceDescriptor {
                interface_number: 1,
                settings: vec![UsbInterfaceSettingDescriptor {
                    setting_number: 0,
                    class_code: 0,
                    sub_class_code: 0,
                    protocol_code: 0,
                    endpoints: vec![],
                }],
            },
        ],
    };

    kernel_drivers.lock().unwrap().insert(0, true);
    kernel_drivers.lock().unwrap().insert(1, true);

    let mut builder = MockDeviceBuilder::new();
    builder.config_descriptor = config;
    let dev = builder
        .with_kernel_drivers(kernel_drivers.clone())
        .with_claimed_interfaces(claimed_interfaces.clone())
        .build();

    let mut ctx = TestContext::new(dev).await;

    ctx.send_op_req_import("1-2").await?;
    let (status, _) = ctx.read_op_rep_import().await?;
    assert_eq!(status, 0);

    {
        let kd = kernel_drivers.lock().unwrap();
        assert_eq!(kd.get(&0), Some(&false)); // Interface 0 detached
        assert_eq!(kd.get(&1), Some(&false)); // Interface 1 detached

        let ci = claimed_interfaces.lock().unwrap();
        assert!(ci.contains(&0)); // Interface 0 claimed
        assert!(ci.contains(&1)); // Interface 1 claimed
    }

    drop(ctx.client);
    ctx.host_handle.await??;

    {
        let kd = kernel_drivers.lock().unwrap();
        assert_eq!(kd.get(&0), Some(&true)); // Interface 0 re-attached
        assert_eq!(kd.get(&1), Some(&true)); // Interface 1 re-attached

        let ci = claimed_interfaces.lock().unwrap();
        assert!(!ci.contains(&0)); // Interface 0 released
        assert!(!ci.contains(&1)); // Interface 1 released
    }

    Ok(())
}
