{
  description = "Pinned Nix toolchain and native dependencies for Neo Buck2 builds";

  # Stable releases have substantially better binary-cache coverage than the
  # moving unstable branch. This matters when the image is built inside
  # BuildBuddy, where uncached compatibility packages cannot use every syscall
  # available to a normal Nix builder.
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";
  inputs.rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

  outputs = { self, nixpkgs, rust-overlay, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config = {
          allowUnfree = true;
          android_sdk.accept_license = true;
        };
        overlays = [ rust-overlay.overlays.default ];
      };
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [
          "aarch64-linux-android"
          "x86_64-linux-android"
        ];
      };
      pixiewoodPerl = pkgs.perl.withPackages (perl: with perl; [
        Glib
        GlibObjectIntrospection
        IPCRun
        JSON
        SetScalar
        XMLLibXML
        XMLLibXSLT
      ]);
      pixiewood = pkgs.stdenvNoCC.mkDerivation {
        pname = "pixiewood";
        version = "482e8aba2158a40f3bccdf24022c883da3303910";
        src = pkgs.fetchurl {
          url = "https://github.com/sp1ritCS/gtk-android-builder/archive/482e8aba2158a40f3bccdf24022c883da3303910.tar.gz";
          hash = "sha256-s2gYtBUXfAO/pPo+Y2xQXYmg38sU3wRgqvZfznt1D/I=";
        };
        installPhase = ''
          mkdir -p "$out/libexec" "$out/bin"
          cp -R . "$out/libexec/pixiewood"
          substituteInPlace "$out/libexec/pixiewood/pixiewood" \
            --replace-fail '#!/usr/bin/perl' '#!${pixiewoodPerl}/bin/perl'
          chmod +x "$out/libexec/pixiewood/pixiewood"
          ln -s "$out/libexec/pixiewood/pixiewood" "$out/bin/pixiewood"
        '';
      };
      androidComponents = pkgs.androidenv.composeAndroidPackages {
        platformVersions = [ "35" ];
        buildToolsVersions = [];
        includeNDK = true;
        ndkVersions = [ "27.2.12479018" ];
      };
      androidBuildTools = pkgs.stdenvNoCC.mkDerivation {
        pname = "android-build-tools";
        version = "35.0.0";
        src = pkgs.fetchurl {
          url = "https://dl.google.com/android/repository/build-tools_r35_linux.zip";
          hash = "sha256-vTpJZpEuuLMO0NALDNprZUO5SdX/4AvqVMBMgeFWHYg=";
        };
        nativeBuildInputs = [ pkgs.unzip ];
        unpackPhase = "unzip $src";
        installPhase = ''
          mkdir -p "$out/libexec/android-sdk/build-tools/35.0.0"
          cp -R android-15/. "$out/libexec/android-sdk/build-tools/35.0.0/"
        '';
      };
      androidSdk = pkgs.runCommand "android-sdk" {} ''
        mkdir -p "$out/libexec/android-sdk"
        for component in ${androidComponents.androidsdk}/libexec/android-sdk/*; do
          if [[ "$(basename "$component")" != build-tools ]]; then
            ln -s "$component" "$out/libexec/android-sdk/"
          fi
        done
        ln -s ${androidBuildTools}/libexec/android-sdk/build-tools \
          "$out/libexec/android-sdk/build-tools"
      '';
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
              appstream
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
              gnutar
              gnugrep
              gnused
              graphene
              gtk4
              gzip
              harfbuzz
              libadwaita
              libxml2
              libxkbcommon
              lld
              ninja
              openssl
              pango
              perl
              pixiewood
              pkg-config
              python3
              rustToolchain
              sassc
              shaderc
              sqlite
              temurin-bin-17
              vulkan-loader
              which
              xz
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

        neo-rbe-image = pkgs.dockerTools.buildLayeredImage {
          name = "ghcr.io/codegod100/neo-rbe";
          tag = "latest";
          contents = [
            androidSdk
            neo-rbe-env
            pkgs.cacert
          ];
          extraCommands = ''
            mkdir -p lib64 opt tmp usr/bin
            chmod 1777 tmp
            ln -s ${pkgs.glibc}/lib/ld-linux-x86-64.so.2 lib64/ld-linux-x86-64.so.2
            ln -s ${androidSdk}/libexec/android-sdk opt/android-sdk
            ln -s /bin/env usr/bin/env
          '';
          config = {
            Env = [
              "ANDROID_HOME=/opt/android-sdk"
              "ANDROID_SDK_ROOT=/opt/android-sdk"
              "GI_TYPELIB_PATH=/lib/girepository-1.0"
              "PATH=/bin:/opt/android-sdk/platform-tools:/opt/android-sdk/cmdline-tools/latest/bin"
            ];
          };
        };

        default = neo-rbe-env;
      };
    };
}
