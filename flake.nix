{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
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
      naersk,
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
        naersk-lib = pkgs.callPackage naersk { };
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
        defaultPackage = naersk-lib.buildPackage {
          pname = "agg";
          src = ./.;
        };

        defaultApp = utils.lib.mkApp { drv = self.defaultPackage."${system}"; };

        devShells.default = mkDevShell pkgs.rust-bin.stable.latest.default;
        devShells.msrv = mkDevShell pkgs.rust-bin.stable.${msrv}.default;
      }
    );
}
