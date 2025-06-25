{
  description = "Super Gametable";

  inputs = {
    nixpkgs.url = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    libmahjong.url = "github:realliance/libmahjong/next";
    crane.url = "github:ipetkov/crane";
  };

  # Based on https://github.com/oxalica/rust-overlay
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      crane,
      libmahjong,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
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
          pkgs.openssl # for rustls
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;

        # build artifacts
        commonArgs = {
          inherit src nativeBuildInputs buildInputs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        unstrippedBin = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );

        bin = pkgs.stdenv.mkDerivation {
          name = "${unstrippedBin.name}-stripped";
          src = unstrippedBin;
          nativeBuildInputs = [ pkgs.binutils ]; # for 'strip'
          installPhase = ''
            mkdir -p $out/bin
            strip -o $out/bin/super-gametable $src/bin/super-gametable
          '';
        };

        dockerImage = pkgs.dockerTools.streamLayeredImage {
          name = "super-gametable";
          tag = "latest";
          contents = [
            bin
            pkgs.cacert
          ];
          config = {
            Cmd = [ "${bin}/bin/super-gametable" ];
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
            just
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
