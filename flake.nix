{
  description = "spotatui — keyboard-driven Spotify TUI with built-in playback";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
          cmake  # some librespot transitive deps need this
        ];

        buildInputs = with pkgs; [
          openssl
          dbus
          libsecret

          # Audio — librespot's rodio backend uses CPAL which talks to ALSA.
          # PipeWire or PulseAudio provide ALSA compat on most NixOS setups.
          alsa-lib
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "spotatui";
          version = "0.2.0";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
            allowBuiltinFetchGit = true;
          };
          inherit nativeBuildInputs buildInputs;
        };

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs buildInputs;
          RUST_BACKTRACE = "1";
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
        };
      }
    );
}
