{
  description = "Hyprland window switcher + app launcher with vim keybinds";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };
  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      crane,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
            "clippy"
            "rustfmt"
          ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        dlopenLibs = with pkgs; [
          libxkbcommon
          vulkan-loader
          wayland
        ];

        buildDeps = with pkgs; [
          wayland
          pkg-config
          openssl
        ];

        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              baseName = builtins.baseNameOf path;
            in
            pkgs.lib.cleanSourceFilter path type && baseName != "target" && baseName != "result";
        };

        commonArgs = {
          inherit src;

          nativeBuildInputs = with pkgs; [
            pkg-config
            wayland-scanner
            makeWrapper
          ];

          buildInputs = buildDeps ++ dlopenLibs;

          WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
          WAYLAND_SCANNER = "${pkgs.wayland-scanner}/bin/wayland-scanner";
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        hyprmwh = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;

            postFixup = ''
              wrapProgram $out/bin/hyprmwh \
                --set LD_LIBRARY_PATH "${pkgs.lib.makeLibraryPath dlopenLibs}" \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.hyprland ]}
            '';
          }
        );
      in
      {
        packages.default = hyprmwh;
        packages.hyprmwh = hyprmwh;

        apps.default = {
          type = "app";
          program = "${hyprmwh}/bin/hyprmwh";
        };

        devShells.default = pkgs.mkShell {
          name = "hyprmwh-dev";
          nativeBuildInputs = with pkgs; [
            rustToolchain
            pkg-config
            cargo-watch
            sccache
            mold
          ];
          buildInputs = buildDeps ++ dlopenLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath dlopenLibs;
          WAYLAND_PROTOCOLS = "${pkgs.wayland-protocols}/share/wayland-protocols";
          RUSTFLAGS = "-C link-arg=-fuse-ld=mold";
          shellHook = ''
            export WAYLAND_SCANNER="${pkgs.wayland-scanner}/bin/wayland-scanner"
            export RUSTC_WRAPPER="${pkgs.sccache}/bin/sccache"
          '';
        };
      }
    );
}
