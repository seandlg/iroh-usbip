# Use a Single Unified Crate Structure

To maintain simplicity and minimize compilation boilerplate, we decided to structure the codebase as a single unified Rust crate. The package will define a library target (`src/lib.rs`) for the core USBIP state machines, packet structures, and P2P connection logic, and a binary target (`src/main.rs`) for the CLI interface. We will only split this into a Cargo workspace if a future requirement mandates separate packaging or independent release cycles for the core library.
