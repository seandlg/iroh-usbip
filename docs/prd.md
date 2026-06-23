# PRD: Secure P2P USB-over-IP (`iroh-usbip`)

## Problem Statement

Developers, system administrators, and hardware hobbyists often need to access physical USB hardware (e.g., debuggers, serial adapters, smart cards, microcontrollers, storage drives) attached to remote machines. Existing USB-over-IP solutions (like standard USBIP) require complex network configuration, port forwarding, VPNs, or exposing vulnerable ports directly to the public internet. This creates significant friction and security risks for remote work and collaborative hardware development.

## Solution

`iroh-usbip` is a lightweight, zero-configuration CLI tool that tunnels standard USBIP traffic securely over the internet using Iroh peer-to-peer (P2P) networking. By leveraging Iroh's encrypted QUIC connections and NAT-traversal capabilities:
- Hosts can securely share physical USB devices using connection tickets.
- Clients can mount those remote devices as local virtual USB devices with a single command.
- All connections are end-to-end encrypted and bypass firewalls and routers automatically.

## User Stories

1. As a host user, I want to list available physical USB devices on my machine, so that I can identify which device I want to share.
2. As a host user, I want to share a physical USB device using its Bus ID or Vendor/Product ID, so that I can make it accessible to a specific peer.
3. As a host user, I want to obtain a secure connection ticket when sharing a device, so that I can copy-paste it to the client user.
4. As a host user, I want the sharing session to block in my terminal, so that I can easily see connection logs and terminate it with `Ctrl+C`.
5. As a host user, I want the local OS kernel drivers to be automatically detached from the device when the client attaches it, so that the user-space redirector can capture all device interfaces.
6. As a host user, I want the local OS kernel drivers to be automatically re-attached when the client disconnects, so that I can immediately reuse the device locally.
7. As a host user, I want the sharing session to shut down automatically after the client detaches, so that no unauthorized clients can reuse the ticket.
8. As a client user, I want to attach a shared remote USB device using a connection ticket, so that it is mounted on my machine as a local virtual device.
9. As a client user, I want the attachment session to run in the foreground, so that I can interrupt it with `Ctrl+C` to cleanly detach the device and close the tunnel.
10. As a client user, I want the network traffic between my machine and the host to be encrypted end-to-end, so that sensitive data (e.g., keystrokes, storage contents) is protected.
11. As a developer, I want to run automated tests for the entire handshake and packet-forwarding loop without having physical USB devices plugged into my CI/CD runner, so that we can prevent regressions.

## Implementation Decisions

- **Host Engine:** Written in Rust, running in user-space using the `rusb` library to communicate with physical USB devices.
- **Client Engine:** Interacts with OS virtual USB controller drivers. On Linux, this is done directly via the `vhci-hcd` sysfs/ioctl attributes. On Windows, this is done experimentally via subprocess integration with the Microsoft WHLK-certified `usbip-win2` driver client CLI (`usbip.exe`).
- **Driverless Host Capability:** Unlike native USBIP, `iroh-usbip` requires **no host-side kernel drivers** (such as `usbip-host`) to share USB devices. It communicates with physical USB hardware entirely in user-space using `rusb` (wrapping `libusb`). The client machine still requires OS-native controller drivers (Linux `vhci-hcd` or Windows `usbip-win2`) to mount virtual devices.
- **Network Transport:** Iroh P2P QUIC streams running on a custom ALPN. The client daemon hosts a local loopback TCP port to interface with the local OS kernel, proxying the bytes transparently to the Iroh stream.
- **Wire Format:** Tunnel raw, un-modified USBIP protocol packets (headers and payloads) directly over Iroh streams.
- **Authentication & Authorization:** Connection tickets act as secret capabilities. Connections are restricted to a single session per ticket, terminating the sharing daemon on disconnect.

## Testing Decisions

- **Trait Abstraction Seam:** The core protocol parser and Host/Client loop will depend on a `UsbDevice` trait. Unit and integration tests will use a mock implementation of this trait to verify proper handling of URBs, device listings, and error states.
- **In-Memory Transport:** Integration tests will bypass the P2P network layer by running the client/host components against a loopback duplex memory stream.
- **Test Scopes:**
  - Standard compliance of the USBIP Control Phase (`OP_REQ_DEVLIST`, `OP_REQ_IMPORT`).
  - Standard compliance of the USBIP Transfer Phase (`USBIP_CMD_SUBMIT`, `USBIP_RET_SUBMIT`).
  - Correct execution of the connection lifecycle (disconnection triggering host teardown).

## Out of Scope

- Multi-client simultaneous mounting of the same physical USB device.
- macOS client support (mounting remote devices on macOS).
- Native kernel host-side bindings (e.g., using `usbip-host` kernel module).
- Persistent daemon mode/PID tracking in the CLI (backgrounding is left to system tools like `systemd` or `tmux`).

