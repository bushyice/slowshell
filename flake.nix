{
  description = "slowshell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane/v0.24.0";
  };

  outputs =
    { nixpkgs, crane, ... }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "aarch64-linux"
        "x86_64-linux"
      ];

      eachSystem =
        f:
        let
          perSystem = lib.genAttrs systems f;
        in
        lib.foldl' (
          acc: system:
          lib.mapAttrs (name: value: (acc.${name} or { }) // { ${system} = value; }) perSystem.${system}
        ) { } systems;

      runtimeLibs = pkgs: [
        pkgs.wayland
        pkgs.libxkbcommon
        pkgs.libGL
        pkgs.vulkan-loader
        pkgs.mesa
      ];

      wrapVulkanSetup = pkgs: ''
        if [ -z "$VULKAN_ICD_FILENAMES" ]; then
          _icd_dirs="${pkgs.mesa.driverLink}/share/vulkan/icd.d /run/opengl-driver/share/vulkan/icd.d"
          _icds=""
          for _d in $_icd_dirs; do
            if [ -d "$_d" ]; then
              for _f in "$_d"/*.json; do
                [ -f "$_f" ] && _icds="''${_icds:+$_icds:}$_f"
              done
            fi
          done
          [ -n "$_icds" ] && export VULKAN_ICD_FILENAMES="$_icds"
          unset _icd_dirs _icds _d _f
        fi
      '';
    in
    eachSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;
        libs = runtimeLibs pkgs;

        src = craneLib.cleanCargoSource ./.;

        shellCrate = craneLib.crateNameFromCargoToml {
          cargoToml = ./crates/slowshell/Cargo.toml;
        };

        commonArgs = {
          inherit src;
          strictDeps = true;
          inherit (shellCrate) pname version;

          nativeBuildInputs = [
            pkgs.pkg-config
            pkgs.makeWrapper
          ];

          buildInputs = libs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        slowshell = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;

            doCheck = false;

            postFixup = ''
              wrapProgram $out/bin/slowshell \
                --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath libs}" \
                --run '${wrapVulkanSetup pkgs}'
            '';

            meta = {
              description = "Some shell";
              homepage = "https://tangled.org/bushyice.com/slowshell";
              mainProgram = "slowshell";
              platforms = lib.platforms.linux;
            };
          }
        );
      in
      {
        packages = {
          inherit slowshell;
          default = slowshell;
        };

        apps.default = {
          type = "app";
          program = lib.getExe slowshell;
        };

        checks = {
          inherit slowshell;
        };

        devShells.default = craneLib.devShell {
          packages = [
            pkgs.git
            pkgs.just
            pkgs.pkg-config
            pkgs.rust-analyzer
            pkgs.socat
          ];

          buildInputs = libs;

          LD_LIBRARY_PATH = lib.makeLibraryPath libs;

          shellHook = ''
            ${wrapVulkanSetup pkgs}
            echo "Rust: $(rustc --version)"
            echo "Vulkan ICD: ${"$"}{VULKAN_ICD_FILENAMES:-none found}"
            echo "build the shell with: nix build .#slowshell"
          '';
        };
      }
    );
}
