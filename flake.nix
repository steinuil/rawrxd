{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.fenix.follows = "fenix";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      naersk,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        rustNightly = fenix.packages.${system}.complete.withComponents [
          "cargo"
          "clippy"
          "rust-analyzer"
          "rust-src"
          "rustc"
          "rustfmt"
        ];

        naerskBuildPackage = (pkgs.callPackage naersk { }).buildPackage;
      in
      rec {
        defaultPackage = packages.rawrxd;

        packages.rawrxd = naerskBuildPackage {
          src = ./.;
        };

        devShell = pkgs.mkShell {
          nativeBuildInputs = [
            rustNightly
            pkgs.cargo-fuzz
          ];

          buildInputs = [
            pkgs.lldb
          ];

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.stdenv.cc.cc.lib ];
          RUST_BACKTRACE = 1;
        };
      }
    );
}
