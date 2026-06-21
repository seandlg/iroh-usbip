# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## 0.1.0 - 2026-06-21

### Bug Fixes

* Accept multiple bi-streams on host to support multi-step handshakes (fixes #15) (3fe86fd)

### Documentation

* Add initial architecture and engineering documentation (c0dd588)
* Add ADR 0009 and update README.md and .gitignore (71bfeed)
* Update README.md and AGENTS.md with Nix/Cargo interplay and commit rules (2798241)

### Features

* Scaffold CLI and implement USB device listing (resolves #2) (48f2565)
* Nix flake and Crane build setup (fixes #11) (f0bc48d)
* Task runner (justfile) integration and nix flake check setup (fixes #12) (8d2e561)
* Host-level E2E integration test script (fixes #13) (5a0e370)
* **release**: Release automation via cargo-dist and git-cliff (dbb5a06)
* Rust-internal mock mode for cross-platform E2E tests (fixes #15) (557252c)

### Implementation Details

* Client-side VHCI driver integration (Closes #6) (ef58864)

### Miscellaneous Tasks

* Add GitHub Actions CI pipeline with Lix and Magic Nix Cache (fixes #14) (9b7e15c)

### Other

* Let there be light 💡 (88714b4)
* Centralize device management via HostDeviceRegistry (21c2b81)

### Refactoring

* **protocol**: Restructure transfer phase to typed Rust structs (closes #8) (ad7fe94)
* Introduce UsbipStream and TransferRunner to decouple framing and transfer execution (71feb79)
* Modularize integration tests and introduce TestContext harness (7330948)
* Standardize on ergonomist hybrid task runner workflow (52c6a4e)

