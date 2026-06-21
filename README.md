# iroh-usbip

> **Notice**: This is an AI-engineered project developed using the agentic engineering flows defined in [mattpocock/skills](https://github.com/mattpocock/skills).

Secure P2P USB-over-IP. Tunnel physical USB devices to remote clients over encrypted Iroh P2P streams with zero network configuration.

## Documentation Index

To keep this project highly maintainable and avoid documentation drift, we separate system concerns:

- **Domain Model & Vocabulary**: See [CONTEXT.md](CONTEXT.md) for terminology (*Host*, *Client*, *Physical Device*, *Virtual Device*, *Bridge*).
- **Architectural Decision Records (ADRs)**: See [docs/adr/](docs/adr/) for design histories, including driver detachment and user-space limitations.
- **Product Requirements**: See [docs/prd.md](docs/prd.md) for scoping, goals, and non-goals.
- **Agent and Triage Guidelines**: See [AGENTS.md](AGENTS.md) and [docs/agents/](docs/agents/) for workspace labels and CLI issue tracker patterns.

---

## Prerequisites

### Host Machine (Sharing physical devices)
- **libusb-1.0**: Development files must be installed.
  - *macOS*: `brew install libusb`
  - *Debian/Ubuntu*: `sudo apt-get install libusb-1.0-0-dev`

### Client Machine (Attaching remote devices)
- **Linux**: Requires the `vhci-hcd` kernel module loaded.
  ```bash
  sudo modprobe vhci-hcd
  ```
- **Windows**: Requires a VHCI driver interface (macOS is currently out of scope for clients).

---

## Quick Start

### 1. List physical USB devices on the Host
Find the Vendor/Product IDs or Bus ID of the device you want to share:
```bash
cargo run -- list
```

### 2. Share the device on the Host
Start the sharing server. This detaches any active kernel drivers and blocks in the foreground, printing a connection ticket:
```bash
cargo run -- share --vid 1d6b --pid 0002
```
*Output:*
```text
Sharing device 1d6b:0002...
Connection ticket: <TICKET_STRING>
```

### 3. Attach the device on the Client
Using the generated ticket, attach the device as a local virtual USB device:
```bash
cargo run -- attach <TICKET_STRING>
```
Pressing `Ctrl+C` in either terminal will cleanly tear down the session, disconnect the virtual device, and reattach the driver on the Host.

---

## Development and Testing

The codebase uses a trait abstraction layer to mock USB hardware and supports in-memory transport streams to run integration tests without physical devices.

Run the test suite:
```bash
cargo test
```
