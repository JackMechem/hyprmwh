{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        runtimeLibs = with pkgs; [
          wayland
          libxkbcommon
          vulkan-loader
          fontconfig
          freetype
        ];
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "hyprmwh";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            makeWrapper
          ];

          buildInputs = runtimeLibs;

          postInstall = ''
            wrapProgram $out/bin/hyprmwh \
              --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs} \
              --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.hyprland ]}
          '';

          meta = {
            description = "Hyprland window switcher overlay with vim keybinds";
            mainProgram = "hyprmwh";
          };
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            pkg-config
          ];
          buildInputs = runtimeLibs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };
      });
}
