{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      nixpkgs,
      fenix,
      naersk,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      fenixPkgs = fenix.packages.${system};

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

      naerskLib = pkgs.callPackage naersk {
        cargo = rust;
        rustc = rust;
      };
    in
    {
      formatter.${system} = pkgs.nixfmt-tree;

      packages.${system}.default = pkgs.callPackage ./default.nix {
        naersk = naerskLib;
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [
          rust
          pkgs.oxfmt
          pkgs.cargo-insta
          pkgs.nixfmt
        ];
      };
    };
}
