{
  description = "tModLoader Workshop library";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
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
        pkgs = nixpkgs.legacyPackages.${system};
        rustToolchain = fenix.packages.${system}.stable.toolchain;
        naersk' = pkgs.callPackage "${naersk}/default.nix" {
          cargo = rustToolchain;
          rustc = rustToolchain;
          clippy = rustToolchain;
        };
      in
      {
        packages.default = naersk'.buildPackage {
          src = ./.;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
          ];
          shellHook = ''
            rustc --version
          '';
        };
      }
    );
}
