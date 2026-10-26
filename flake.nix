{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    # Unstable's rustc 1.96+ (LLVM 21) breaks surrealdb's AVX-512 VNNI intrinsics.
    nixpkgs.url = "github:nixos/nixpkgs/nixos-26.05";
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

          # pkgs.nixos refuses to evaluate on darwin.
          checks = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
            mkModule-secrets =
              let
                inherit (pkgs.lib) any hasInfix;
                dummy = args: lib.mkModule ({ self = null; description = "dummy"; } // args);
                eval = service: pkgs.nixos {
                  imports = [
                    (dummy { pname = "dummy"; secretFields = [ "api_key" "tokens.*.token" ]; })
                    (dummy { pname = "plain"; })
                  ];
                  services.dummy = { enable = true; package = pkgs.emptyDirectory; } // service;
                  boot.loader.grub.enable = false;
                  fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
                  system.stateVersion = pkgs.lib.trivial.release;
                };
                failed = nixos: map (a: a.message)
                  (builtins.filter (a: !a.assertion) nixos.config.assertions);
                creds = "/run/credentials/dummy.service";
                good = eval {
                  settings.tokens.phone.name = "phone";
                  secrets = {
                    api_key = "/run/secrets/key";
                    "tokens.phone.token" = "/run/secrets/phone";
                  };
                };
                bad = failed (eval {
                  settings.tokens.phone.token.plaintext = "hunter2";
                  secrets = {
                    "tokens.phone.tokn" = "/run/secrets/phone";
                    "tokens.a:b.token" = "/run/secrets/ab";
                  };
                });
              in
              assert failed good == [ ];
              assert good.config.services.dummy.settings == {
                api_key.file = "${creds}/api_key";
                tokens.phone = { name = "phone"; token.file = "${creds}/tokens.phone.token"; };
              };
              assert good.config.systemd.services.dummy.serviceConfig.LoadCredential
                == [ "api_key:/run/secrets/key" "tokens.phone.token:/run/secrets/phone" ];
              assert builtins.length bad == 3;
              assert any (hasInfix "settings.tokens.phone.token: plaintext") bad;
              assert any (hasInfix "secrets.\"tokens.phone.tokn\" matches no secret field") bad;
              assert any (hasInfix "secrets.\"tokens.a:b.token\" matches no secret field") bad;
              assert good.options.services.dummy ? secrets;
              assert !(good.options.services.plain ? secrets);
              pkgs.runCommand "mkModule-secrets" { } "touch $out";
          };
        })
    // { inherit lib; };
}
