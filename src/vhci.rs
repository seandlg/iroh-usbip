use std::fs;
use std::path::{Path, PathBuf};
use crate::UsbSpeed;

#[cfg(unix)]
pub type RawFd = std::os::unix::io::RawFd;
#[cfg(not(unix))]
pub type RawFd = i32;

/// Controller for interacting with the Linux kernel's Virtual Host Controller Interface (vhci-hcd) driver.
#[derive(Debug, Clone)]
pub struct VhciController {
    sysfs_path: PathBuf,
}

/// Find the active VHCI sysfs directory by checking common platform driver locations.
pub fn find_vhci_dir() -> Option<PathBuf> {
    let candidates = [
        "/sys/devices/platform/vhci_hcd.0",
        "/sys/devices/platform/vhci-hcd.0",
        "/sys/devices/platform/vhci_hcd",
        "/sys/devices/platform/vhci-hcd",
    ];
    for path in &candidates {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }
    None
}

impl VhciController {
    /// Create a new VHCI controller detecting the sysfs path on the system.
    pub fn new() -> Self {
        let path = find_vhci_dir()
            .unwrap_or_else(|| PathBuf::from("/sys/devices/platform/vhci_hcd.0"));
        Self { sysfs_path: path }
    }

    /// Create a VHCI controller pointing to a specific sysfs path (useful for testing/mocking).
    pub fn with_path(path: PathBuf) -> Self {
        Self { sysfs_path: path }
    }

    /// Check if the VHCI kernel driver is available.
    pub fn is_available(&self) -> bool {
        self.sysfs_path.exists()
    }

    /// Find the first free port on the VHCI controller.
    pub fn find_free_port(&self) -> anyhow::Result<u32> {
        let mut status_files = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.sysfs_path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == "status" || name.starts_with("status.") {
                    status_files.push(entry.path());
                }
            }
        }
        if status_files.is_empty() {
            status_files.push(self.sysfs_path.join("status"));
        }

        for status_path in status_files {
            let content = match fs::read_to_string(&status_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for line in content.lines() {
                if line.contains("hub") || line.contains("port") || line.contains("sta") {
                    continue;
                }

                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    if let (Ok(port), Ok(status), Ok(sockfd)) = (
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                        parts[5].parse::<u32>(),
                    ) {
                        // Port is free if status is VDEV_ST_NULL (4) or VDEV_ST_NOTASSIGNED (5) or sockfd is 0
                        if status == 4 || status == 5 || sockfd == 0 {
                            return Ok(port);
                        }
                    }
                }
            }
        }

        anyhow::bail!("No free VHCI ports available or status file missing/unreadable")
    }

    /// Attach a socket file descriptor to a virtual port.
    pub fn attach(&self, port: u32, sockfd: RawFd, devid: u32, speed: UsbSpeed) -> anyhow::Result<()> {
        let speed_code = crate::protocol::map_speed(speed);
        let cmd = format!("{} {} {} {}\n", port, sockfd, devid, speed_code);
        let attach_path = self.sysfs_path.join("attach");
        fs::write(&attach_path, cmd)
            .map_err(|e| anyhow::anyhow!("Failed to write to {}: {}", attach_path.display(), e))?;
        Ok(())
    }

    /// Detach a virtual port.
    pub fn detach(&self, port: u32) -> anyhow::Result<()> {
        let cmd = format!("{}\n", port);
        let detach_path = self.sysfs_path.join("detach");
        fs::write(&detach_path, cmd)
            .map_err(|e| anyhow::anyhow!("Failed to write to {}: {}", detach_path.display(), e))?;
        Ok(())
    }
}
