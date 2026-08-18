{
  description = "parrotui-spotify — keyboard-driven Spotify TUI with built-in playback";

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

        buildInputs = with pkgs; lib.optionals stdenv.isLinux [
          dbus
          libsecret
          alsa-lib
        ] ++ lib.optionals stdenv.isDarwin (with darwin.apple_sdk.frameworks; [
          AudioUnit
          CoreAudio
          CoreServices
          Security
          SystemConfiguration
        ]);
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "parrotui-spotify";
          version = "0.40.0";
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
