use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use iroh_usbip::{VhciController, UsbSpeed};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn setup_temp_sysfs() -> PathBuf {
    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let sysfs_path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join(format!("test_sysfs_{}_{}", std::process::id(), counter));
    if sysfs_path.exists() {
        let _ = fs::remove_dir_all(&sysfs_path);
    }
    fs::create_dir_all(&sysfs_path).unwrap();
    sysfs_path
}

#[test]
fn test_vhci_availability() {
    let nonexistent = PathBuf::from("/nonexistent/sysfs/path/vhci");
    let controller = VhciController::with_path(nonexistent);
    assert!(!controller.is_available());

    let temp_path = setup_temp_sysfs();
    let controller = VhciController::with_path(temp_path.clone());
    assert!(controller.is_available());

    let _ = fs::remove_dir_all(&temp_path);
}

#[test]
fn test_vhci_find_free_port() {
    let temp_path = setup_temp_sysfs();
    let controller = VhciController::with_path(temp_path.clone());

    // 1. With status file missing/empty
    assert!(controller.find_free_port().is_err());

    // 2. Write status file with some used and some free ports
    let status_content = "\
hub port sta spd dev sockfd local_busid
hs 0000 006 000 00010002 000005 1-1
hs 0001 004 000 00000000 000000 0-0
hs 0002 006 000 00010003 000006 1-2
ss 0003 004 000 00000000 000000 0-0
";
    fs::write(temp_path.join("status"), status_content).unwrap();

    let free_port = controller.find_free_port().unwrap();
    assert_eq!(free_port, 1);

    // 3. Write status file with all ports used
    let all_used_content = "\
hub port sta spd dev sockfd local_busid
hs 0000 006 000 00010002 000005 1-1
hs 0001 006 000 00010003 000006 1-2
";
    fs::write(temp_path.join("status"), all_used_content).unwrap();
    assert!(controller.find_free_port().is_err());

    let _ = fs::remove_dir_all(&temp_path);
}

#[test]
fn test_vhci_attach() {
    let temp_path = setup_temp_sysfs();
    let controller = VhciController::with_path(temp_path.clone());

    controller.attach(3, 42, 0x00010005, UsbSpeed::High).unwrap();

    let attach_file = temp_path.join("attach");
    assert!(attach_file.exists());
    let content = fs::read_to_string(attach_file).unwrap();
    assert_eq!(content.trim(), "3 42 65541 3"); // 0x00010005 = 65541, UsbSpeed::High = 3

    let _ = fs::remove_dir_all(&temp_path);
}

#[test]
fn test_vhci_detach() {
    let temp_path = setup_temp_sysfs();
    let controller = VhciController::with_path(temp_path.clone());

    controller.detach(2).unwrap();

    let detach_file = temp_path.join("detach");
    assert!(detach_file.exists());
    let content = fs::read_to_string(detach_file).unwrap();
    assert_eq!(content.trim(), "2");

    let _ = fs::remove_dir_all(&temp_path);
}
