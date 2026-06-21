{
  description = "Securely share and access USB devices over the internet using Iroh and USBIP";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Clean source using Crane's helper
        src = craneLib.cleanCargoSource ./.;

        # Define build arguments shared between dependency build and actual package
        commonArgs = {
          inherit src;
          strictDeps = true;

          nativeBuildInputs = [
            pkgs.pkg-config
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            (pkgs.writeShellScriptBin "sw_vers" ''
              echo "14.5"
            '')
          ];

          buildInputs = [
            pkgs.libusb1
          ];
        };

        # Cache dependencies separately
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "iroh-usbip-deps";
        });

        # Build the actual crate
        iroh-usbip = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        packages.default = iroh-usbip;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ iroh-usbip ];
          packages = [
            rustToolchain
            pkgs.pkg-config
            pkgs.libusb1
          ];
        };
      });
}
