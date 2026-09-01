{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { nixpkgs, fenix, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      fenixPkgs = fenix.packages.${system};

      # Prebuilt release binary; nixpkgs' oxfmt lags the version we want.
      oxfmt = pkgs.stdenv.mkDerivation {
        pname = "oxfmt";
        version = "0.65.0";

        src = pkgs.fetchurl {
          url = "https://github.com/oxc-project/oxc/releases/download/apps_v1.80.0/oxfmt-x86_64-unknown-linux-gnu.tar.gz";
          hash = "sha256-tcZA1XVb6oU1hFSIB8lG12ohosgMaQLo+T1YLXOWlak=";
        };

        sourceRoot = ".";

        nativeBuildInputs = [ pkgs.autoPatchelfHook ];
        buildInputs = [ pkgs.stdenv.cc.cc.lib ]; # libgcc_s.so.1

        installPhase = ''
          runHook preInstall
          install -Dm755 oxfmt-x86_64-unknown-linux-gnu $out/bin/oxfmt
          runHook postInstall
        '';

        meta.mainProgram = "oxfmt";
      };

      rust = fenixPkgs.combine [
        (fenixPkgs.stable.withComponents [
          "cargo"
          "rustc"
          "rust-std"
          "clippy"
          "rust-analyzer"
          "rust-src"
        ])
        # rustfmt.toml uses nightly-only options
        fenixPkgs.default.rustfmt
      ];
    in
    {
      formatter.${system} = pkgs.nixfmt-tree;

      devShells.${system}.default = pkgs.mkShell {
        packages = [
          oxfmt
          rust
          pkgs.nixfmt
        ];
      };
    };
}
