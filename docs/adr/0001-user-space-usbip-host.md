# Use User-space USBIP Host Redirection

To support sharing USB devices from macOS, Windows, and Linux without requiring OS-specific kernel drivers on the host, we decided to implement a user-space USBIP host daemon. We will use the Rust `rusb` (libusb wrapper) crate to communicate with physical USB devices. This allows a unified cross-platform Host implementation, though it may introduce slightly higher latency compared to a kernel-level driver for extremely high-throughput devices.
