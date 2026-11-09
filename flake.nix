{
  description = "A small Datalog: a parser with a line and column on every refusal, and an evaluator on datafrog";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAll =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            };
            # One Rust for building, checking and working here.
            toolchain = pkgs.rust-bin.stable."1.99.1".default.override {
              extensions = [
                "clippy"
                "rustfmt"
                "rust-src"
              ];
            };
            craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              # The cases are TOML and the specs are Markdown, and the tests
              # read both, so the source is more than Cargo's own files.
              filter =
                path: type:
                (craneLib.filterCargoSources path type)
                || pkgs.lib.hasSuffix ".toml" path
                || pkgs.lib.hasSuffix ".md" path;
            };
            common = {
              inherit src;
              strictDeps = true;
              pname = "datalog";
              version = "0.1.0";
            };
            cargoArtifacts = craneLib.buildDepsOnly common;
          in
          f {
            inherit
              pkgs
              toolchain
              craneLib
              common
              cargoArtifacts
              ;
          }
        );
    in
    {
      packages = forAll (
        { craneLib, common, cargoArtifacts, ... }:
        {
          # The crates built, with every test run.
          default = craneLib.buildPackage (common // { inherit cargoArtifacts; });
        }
      );

      checks = forAll (
        { craneLib, common, cargoArtifacts, ... }:
        {
          # The same derivation as packages.default, so it is built once.
          build = craneLib.buildPackage (common // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (common // { inherit cargoArtifacts; });
          clippy = craneLib.cargoClippy (
            common
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
            }
          );
          fmt = craneLib.cargoFmt { inherit (common) src pname version; };
        }
      );

      devShells = forAll (
        { pkgs, toolchain, ... }:
        {
          default = pkgs.mkShell { packages = [ toolchain ]; };
        }
      );

      formatter = forAll ({ pkgs, ... }: pkgs.nixfmt);
    };
}
