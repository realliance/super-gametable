{
  description = "Small Turtle Game Server";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
    libmahjong.url = "github:realliance/libmahjong/next";
    crane.url = "github:ipetkov/crane";
  };

  # Based on https://github.com/oxalica/rust-overlay
  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane, libmahjong, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # Input pkgs
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Setup crane with toolchain
        rustToolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # crane define src
        src = craneLib.cleanCargoSource ./.;

        # Get libmahjong-gcc package from the flake
        libmahjongPkg = libmahjong.packages.${system}.gcc;

        nativeBuildInputs = [
          pkgs.pkg-config
        ];
        
        buildInputs = [
          libmahjongPkg
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

        # build artifacts
        commonArgs = {
          inherit src nativeBuildInputs buildInputs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        bin = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        # Add Docker image configuration
        dockerImage = pkgs.dockerTools.streamLayeredImage {
          name = "sm-turtle-game";
          tag = "latest";
          contents = [ bin ];
          config = {
            Cmd = [ "${bin}/bin/sm-turtle-game" ]; # Adjust binary name as needed
          };
        };
    in
    with pkgs;
    {
      devShells.default = mkShell {
        inherit LD_LIBRARY_PATH;
        buildInputs = [
          rustToolchain
          libmahjongPkg
          dive # Tool for exploring Docker images
        ];
        nativeBuildInputs = [
          pkg-config
        ];
      };
      packages = {
        inherit bin dockerImage;
        default = bin;
      };
    }
  );
}
