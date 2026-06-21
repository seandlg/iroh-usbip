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

Before running any commands, enter the reproducible development shell (which automatically loads all system dependencies like `libusb` and `pkg-config`):
```bash
nix develop
```
*(If Nix is not installed, verify you have installed the manual prerequisites listed above.)*

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
- **Reproducible Shell**: Run `nix develop` (or configure `direnv` with `use flake`) to enter a development shell containing all toolchains, libraries (`libusb1`, `pkg-config`), and tools like `just`, `gh`, and `git-cliff`.
- **Idiomatic Cargo Flow**: Once inside `nix develop`, run standard Cargo commands directly. Spawning nested Nix subshells (like `nix develop --command cargo`) is avoided for local runs to keep feedback loops fast and IDE integrations (like Rust-Analyzer) working flawlessly.
- **Task Runner (`just`)**: We reserve the `justfile` purely for complex, multi-step orchestrations or privilege transitions (e.g. running kernel integration tests and release pipelines).

### 2. Testing Scopes
We separate testing into two distinct environments and privilege scopes:

- **Hermetic Checks (Mock Unit & Integration Tests)**
  These tests run without any physical USB hardware or host-level kernel permissions. They use in-memory stream mocks and mock devices.
  - Run clippy and format checks:
    ```bash
    cargo fmt --all --check
    cargo clippy --all-targets -- --deny warnings
    ```
  - Run all hermetic unit and mock integration tests:
    ```bash
    cargo test
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
    *Note:* The runner automatically compiles the binary as the normal user first, then runs `scripts/e2e.sh` using `sudo -E env PATH="$PATH"` to preserve the Nix-provided dependencies for the root shell, avoiding permission pollution in the build target directory.
  - Run the E2E test in Mock Mode (does not require root or Linux VHCI):
    ```bash
    just test-e2e --mock
    ```

### 3. Continuous Integration (CI)
Our GitHub Actions pipeline (defined in `.github/workflows/ci.yml`) runs on every commit/PR:
1. **Nix Setup**: Installs Nix/Lix and configures **Magic Nix Cache** for incremental store caching.
2. **Hermetic Checks**: Executes `nix flake check` to verify formatting, clippy, and unit tests inside the sandbox.
3. **E2E Testing**: Runs the unsandboxed mock E2E integration tests using `nix develop --command scripts/e2e.sh --mock`.

### 4. Releasing (Automation & SemVer)
We use a double-gated, mistake-proof release workflow built around `cargo-dist` and `git-cliff`. All release actions must be run inside `nix develop`.

#### **How to release:**
1. **Prepare Release** (from a clean `main` branch inside `nix develop`):
   Determine the next version according to Semantic Versioning (SemVer) rules:
   - **Patch** (`0.1.x`): For bug fixes, refactorings, chores, and internal improvements.
   - **Minor** (`0.x.0`): For new features (e.g. support for a new command).
   - **Major** (`x.0.0`): For breaking changes.
   
   Run the task runner recipe to prepare the release:
   ```bash
   just prepare-release <version>
   ```
   *Gate 1 (Poka-Yoke):* This will fail if the latest commit on `main` has not passed GitHub Actions CI. If green, it creates a `release/v<version>` branch, bumps the version in `Cargo.toml`, updates `CHANGELOG.md` via `git-cliff`, and commits the changes.
   
2. **Submit PR & Merge**:
   Push the `release/v<version>` branch to GitHub, open a PR, and merge it to `main` once PR checks (including E2E checks) succeed.
   
3. **Tag and Publish** (inside `nix develop`):
   Pull the merged commit locally on `main` and run:
   ```bash
   just tag-release
   ```
   *Gate 2 (Poka-Yoke):* This will fail if the post-merge CI on `main` hasn't completed successfully yet. If green, it creates the annotated git tag `v<version>` and pushes it, which triggers `cargo-dist` in CI to compile binaries, package installers, and publish the GitHub Release.


