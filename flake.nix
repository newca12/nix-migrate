{
  description = "nix-migrate development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, utils, rust-overlay, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Parse Cargo.toml dynamically
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        rustVersion = cargoToml.package.rust-version or "latest";

        baseRustToolchain = (
          if rustVersion == "latest" then
            pkgs.rust-bin.stable.latest.default
          else
            pkgs.rust-bin.stable.${rustVersion}.default
        );

        devRustToolchain = baseRustToolchain.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = baseRustToolchain;
          rustc = baseRustToolchain;
        };

        nix-migrate = rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          src = pkgs.lib.cleanSource ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          meta = with pkgs.lib; {
            description = "High-performance Nix migration MCP server and scanner";
            homepage = "https://github.com/newca12/nix-migrate";
            mainProgram = "nix-migrate";
            license = licenses.mit;
            platforms = platforms.unix;
          };
        };
      in {
        packages.default = nix-migrate;
        packages.nix-migrate = nix-migrate;

        apps.default = {
          type = "app";
          program = "${nix-migrate}/bin/nix-migrate";
        };
        apps.nix-migrate = {
          type = "app";
          program = "${nix-migrate}/bin/nix-migrate";
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            devRustToolchain
          ];
        };
      });
}
