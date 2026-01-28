{
  description = "OpenFlight (Flight Hub) development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        lib = pkgs.lib;

        rustToolchain = pkgs.rust-bin.stable."1.89.0".default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

        linuxDeps = lib.optionals pkgs.stdenv.isLinux [
          pkgs.libusb1
          pkgs.systemd
        ];

        darwinDeps = lib.optionals pkgs.stdenv.isDarwin [
          pkgs.libusb1
          pkgs.libiconv
        ];

        devTools = [
          pkgs.pkg-config
          pkgs.protobuf
          pkgs.git
          pkgs.gnumake
          pkgs.rust-analyzer
          pkgs.cargo-deny
          pkgs.cargo-audit
          pkgs.cargo-nextest
          pkgs.cargo-watch
          pkgs.cargo-public-api
        ];
      in {
        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain ] ++ devTools ++ linuxDeps ++ darwinDeps;

          PROTOC = "${pkgs.protobuf}/bin/protoc";
          RUST_LOG = "info";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        formatter = pkgs.alejandra;
      });
}
