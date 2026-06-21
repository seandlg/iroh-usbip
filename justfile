# iroh-usbip task runner
#
# Standardizes development tasks, wrapping commands with nix develop.

# Run clippy and format check inside the Nix environment
check:
    nix develop --command cargo fmt --all --check
    nix develop --command cargo clippy --all-targets -- --deny warnings

# Build the binaries using Nix
build:
    nix build

# Run the unit and mock integration tests inside the Nix environment
test:
    nix develop --command cargo test
