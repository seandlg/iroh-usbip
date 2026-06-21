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

## Development, Testing, and CI

This repository uses a combination of **Cargo** and **Nix** (specifically the **Lix** implementation with the **Crane** packaging library) to manage hermetic dependencies, build environments, and cached builds.

### 1. Nix & Cargo Interplay
- **Reproducible Shell**: Run `nix develop` to enter a development shell containing all toolchains, libraries (`libusb1`, `pkg-config`), and tools like `just`.
- **Cached Builds**: Crane compiles and caches Rust/Cargo dependencies separately from the project source to keep rebuilds fast.
- **Task Runner (`just`)**: We standardize common development tasks using a `justfile`. These targets automatically execute commands wrapped in the Nix development environment (e.g., running `nix develop --command ...`).

### 2. Testing Scopes
We separate testing into two distinct environments and privilege scopes:

- **Hermetic Checks (Mock Unit & Integration Tests)**
  These tests run without any physical USB hardware or host-level kernel permissions. They use in-memory stream mocks and mock devices.
  - Run clippy and format check:
    ```bash
    just check
    ```
  - Run all hermetic unit and mock integration tests:
    ```bash
    just test
    ```
  - Nix hermetic check (runs clippy, formatting, and unit tests inside a sandboxed build derivation):
    ```bash
    nix flake check
    ```

- **Native E2E Integration Tests (Linux Only)**
  These tests verify real kernel integration against the Linux Virtual Host Controller Interface (VHCI) driver. Because they load kernel modules (`vhci-hcd`, `dummy-hcd`, `libcomposite`) and configure configfs/sysfs, they **must run natively on the host** with root/sudo privileges.
  - Run the E2E integration test:
    ```bash
    just test-e2e
    ```
  - This executes [scripts/e2e.sh](scripts/e2e.sh), which dynamically configures a virtual USB gadget, shares it via the host daemon, attaches it via the client daemon, and verifies the connection.

### 3. Continuous Integration (CI)
Our GitHub Actions pipeline (defined in `.github/workflows/ci.yml`) runs on every commit/PR:
1. **Nix Setup**: Installs Nix/Lix and configures **Magic Nix Cache** for incremental store caching.
2. **Hermetic Checks**: Executes `nix flake check` to verify formatting, clippy, and unit tests inside the sandbox.
3. **E2E Testing**: Runs the unsandboxed E2E integration tests directly on the runner host using `sudo nix develop --command scripts/e2e.sh`.

