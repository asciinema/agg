{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      utils,
    }:
    let
      packageToml = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
      msrv = packageToml.rust-version;
    in
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        rust = pkgs.rust-bin.stable.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
        mkDevShell =
          rust:
          pkgs.mkShell {
            nativeBuildInputs = [
              (rust.override {
                extensions = [
                  "rustfmt"
                  "rust-src"
                  "rust-analyzer"
                  "clippy"
                ];
              })
              pkgs.pre-commit
            ];
          };
      in
      {
        packages.default = rustPlatform.buildRustPackage {
          pname = packageToml.name;
          version = packageToml.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
        };

        apps.default = utils.lib.mkApp { drv = self.packages.${system}.default; };

        devShells.default = mkDevShell rust;
        devShells.msrv = mkDevShell pkgs.rust-bin.stable.${msrv}.default;
      }
    );
}
