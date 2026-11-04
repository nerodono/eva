{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { flake-utils, fenix, nixpkgs, crane, ... }:
    let
      lib = import ./nix/lib.nix { inherit nixpkgs fenix flake-utils crane; };
    in
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = lib.mkPkgs system;
        in
        {
          devShells.default = lib.mkDevShell { inherit pkgs; };

          checks = import ./nix/tests.nix { inherit pkgs; eva = lib; };
        })
    // { inherit lib; };
}
