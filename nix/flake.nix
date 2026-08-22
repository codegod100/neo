{
  description = "Pinned Nix toolchain and native dependencies for Neo Buck2 builds";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in {
      packages.${system} = rec {
        neo-rbe-env = pkgs.buildEnv {
          name = "neo-rbe-env";
          ignoreCollisions = true;
          extraOutputsToInstall = [ "dev" "lib" ];
          pathsToLink = [
            "/bin"
            "/include"
            "/lib"
            "/share"
          ];
          paths =
            (with pkgs; [
              bash
              binutils
              blueprint-compiler
              cairo
              clang
              clippy
              coreutils
              file
              findutils
              gawk
              gdk-pixbuf
              gitMinimal
              glib
              gnumake
              gnugrep
              gnused
              graphene
              gtk4
              harfbuzz
              libadwaita
              libxkbcommon
              lld
              openssl
              pango
              perl
              pkg-config
              python3
              rustc
              sqlite
              vulkan-loader
              which
              zlib
            ])
            ++ map pkgs.lib.getLib (with pkgs; [
              cairo
              gdk-pixbuf
              glib
              graphene
              gtk4
              harfbuzz
              libadwaita
              libxkbcommon
              openssl
              pango
              sqlite
              vulkan-loader
              zlib
            ]);
        };

        default = neo-rbe-env;
      };
    };
}
