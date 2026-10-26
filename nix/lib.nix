# Shared flake machinery for eva services. Entry point is mkServiceFlake;
# the pieces (mkPkgs, rustToolchain, mkPackage, mkDevShell, mkModule) are
# exported separately for services that need a nonstandard flake.
{ nixpkgs, fenix, flake-utils, crane }:
let
  toolchain = {
    name = "1.95";
    sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
  };
in
rec {
  mkPkgs = system: import nixpkgs {
    inherit system;
    overlays = [ fenix.overlays.default ];
  };

  rustToolchain = pkgs: with pkgs.fenix; combine [
    ((fromToolchainName toolchain).withComponents [
      "cargo"
      "rustc"
      "rust-src"
      "rust-analyzer"
      "clippy"
      "llvm-tools-preview"
    ])
    default.rustfmt
  ];

  mkDevShell =
    { pkgs
    , stdenv ? null
    , extraPackages ? [ ]
    , env ? { }
    }:
    let
      mkShell =
        if stdenv == null
        then pkgs.mkShell
        else pkgs.mkShell.override { inherit stdenv; };
    in
    mkShell (env // {
      packages = [ (rustToolchain pkgs) ] ++ extraPackages;
      buildInputs = [ pkgs.stdenv.cc.cc.lib ];
    });

  # Crane handles git dependencies straight from Cargo.lock — no
  # outputHashes. env goes only into the final build so dependency
  # artifacts stay cached across values like a commit hash.
  mkPackage =
    { pkgs
    , src
    , pname
    , version ? "0.1.0"
    , env ? { }
    , srcFilter ? null
    }:
    let
      craneLib = (crane.mkLib pkgs).overrideToolchain (_: rustToolchain pkgs);
      # The build needs more than Cargo sources: checkPhase compares against
      # insta snapshots, and SQL/SurrealQL migrations are embedded at compile time.
      # Dropped, they fail late and obscurely — snapshot tests with nothing
      # to compare against, every query against a schema-less database.
      buildData = path: builtins.match ".*\\.(snap|sql|surql)$" path != null;
      cleaned = pkgs.lib.cleanSourceWith {
        inherit src;
        filter = path: type:
          craneLib.filterCargoSources path type
          || buildData path
          || (srcFilter != null && srcFilter path type);
        name = "source";
      };
      common = {
        inherit pname version;
        src = cleaned;
        strictDeps = true;
      };
      cargoArtifacts = craneLib.buildDepsOnly (common // {
        # crane also runs `cargo check` by default, for check/clippy derivations
        # that are never built here.
        buildPhaseCargoCommand = "cargoWithProfile build --locked";
      });
    in
    craneLib.buildPackage (common // { inherit cargoArtifacts; } // env);

  # services.<pname>: settings is the non-secret config (rendered into the
  # world-readable nix store). SecretString fields named by secretFields
  # (dotted paths, `*` = one attribute level) take files from `secrets`;
  # other sensitive sections go into extraConfigFiles — the server merges
  # repeated --config files, later over earlier.
  mkModule =
    { self
    , pname
    , description
    , mainProgram ? pname
    , format ? "toml"
    , settingsDefaults ? { }
    , secretFields ? [ ]
    , extraServiceConfig ? { }
    }:
    { config, lib, pkgs, ... }:
    let
      cfg = config.services.${pname};
      fmt = pkgs.formats.${format} { };
      configFile = fmt.generate "${pname}.${format}" cfg.settings;
      configArgs = lib.concatMapStringsSep " "
        (file: "--config ${lib.escapeShellArg file}")
        ([ configFile ] ++ cfg.extraConfigFiles);

      credentials = "/run/credentials/${pname}.service";
      secrets = cfg.secrets or { };
      patterns = map (lib.splitString ".") secretFields;
      isSecretKey = key:
        let path = lib.splitString "." key;
        in lib.any
          (p: lib.length p == lib.length path
            && lib.all lib.id (lib.zipListsWith (s: k: k == s || (s == "*" && builtins.match "[^:/%]+" k != null)) p path))
          patterns;
      # Walks only along the pattern: a generic walk would descend into
      # derivations in settings, which refer to themselves.
      plaintextKeys = path: pattern: v:
        let seg = lib.head pattern; in
        if pattern == [ ] then lib.optional (v ? plaintext) (lib.concatStringsSep "." path)
        else if !builtins.isAttrs v then [ ]
        else lib.concatMap (n: plaintextKeys (path ++ [ n ]) (lib.tail pattern) v.${n})
          (if seg == "*" then lib.attrNames v else lib.optional (v ? ${seg}) seg);
    in
    {
      options.services.${pname} = {
        enable = lib.mkEnableOption description;

        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.${pname};
          defaultText = lib.literalExpression "${pname}.packages.<system>.${pname}";
          description = "${pname} package to run.";
        };

        settings = lib.mkOption {
          type = fmt.type;
          default = { };
          description = ''
            Server configuration. Stored in the world-readable nix store —
            no secrets here: secret fields take files from `secrets`, other
            sensitive sections go into extraConfigFiles.
          '';
        };

        extraConfigFiles = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          example = [ "/run/secrets/${pname}.${format}" ];
          description = ''
            Extra --config files merged over settings at startup, read at
            runtime — sops-nix / agenix secret paths fit. Must be readable
            by the service user.
          '';
        };

        user = lib.mkOption {
          type = lib.types.str;
          default = pname;
          description = ''
            User to run the service as. Not created by this module —
            declare it (or point at an existing one) on the host.
          '';
        };

        group = lib.mkOption {
          type = lib.types.str;
          default = pname;
          description = "Group to run the service as. Not created by this module.";
        };

        environment = lib.mkOption {
          type = lib.types.attrsOf lib.types.str;
          default = { };
          description = "Extra environment variables for the service.";
        };

        environmentFiles = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
          description = "systemd EnvironmentFile entries (e.g. secret env vars).";
        };
      } // lib.optionalAttrs (secretFields != [ ]) {
        secrets = lib.mkOption {
          type = lib.types.attrsOf lib.types.str;
          default = { };
          example = { ${lib.replaceStrings [ "*" ] [ "name" ] (lib.head secretFields)} = "/run/secrets/${pname}"; };
          description = ''
            Secret files by dotted config key matching one of
            ${lib.concatStringsSep ", " secretFields}. systemd hands each file
            over by LoadCredential (a root-only source is enough), and
            settings get `<key>.file = "${credentials}/<key>"`.
          '';
        };
      };

      config = lib.mkIf cfg.enable {
        services.${pname}.settings = lib.mkMerge
          ([ (lib.mapAttrsRecursive (_: lib.mkDefault) settingsDefaults) ]
            ++ lib.mapAttrsToList
            (key: _: lib.setAttrByPath (lib.splitString "." key) { file = "${credentials}/${key}"; })
            secrets);

        assertions =
          map
            (key: {
              assertion = false;
              message = "services.${pname}.settings.${key}: plaintext secrets land in the world-readable nix store; set services.${pname}.secrets.\"${key}\" to the secret file instead";
            })
            (lib.concatMap (p: plaintextKeys [ ] p cfg.settings) patterns)
          ++ lib.mapAttrsToList
            (key: _: {
              assertion = isSecretKey key;
              message = "services.${pname}.secrets.\"${key}\" matches no secret field (${lib.concatStringsSep ", " secretFields}; names may not contain : / %)";
            })
            secrets;

        systemd.services.${pname} = {
          inherit description;
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];
          environment = cfg.environment;
          serviceConfig = lib.mkMerge [
            ({
              User = cfg.user;
              Group = cfg.group;
              StateDirectory = pname;
              ExecStart = "${cfg.package}/bin/${mainProgram} ${configArgs}";
              Restart = "on-failure";
              RestartSec = 5;
              EnvironmentFile = cfg.environmentFiles;
            } // extraServiceConfig)
            # A separate definition, so a LoadCredential in extraServiceConfig
            # concatenates with it instead of replacing it.
            { LoadCredential = lib.mapAttrsToList (key: path: lib.replaceStrings [ "%" ] [ "%%" ] "${key}:${path}") secrets; }
          ];
        };
      };
    };

  # The whole default service flake: packages.{<pname>,default},
  # devShells.default, nixosModules.default. devShell is a function of
  # pkgs returning mkDevShell arguments (minus pkgs).
  mkServiceFlake =
    { self
    , pname
    , description
    , mainProgram ? pname
    , version ? "0.1.0"
    , env ? { }
    , srcFilter ? null
    , format ? "toml"
    , settingsDefaults ? { }
    , secretFields ? [ ]
    , extraServiceConfig ? { }
    , devShell ? (_: { })
    }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = mkPkgs system;
          package = mkPackage {
            inherit pkgs pname version env srcFilter;
            src = self;
          };
        in
        {
          packages = {
            ${pname} = package;
            default = package;
          };
          devShells.default = mkDevShell ({ inherit pkgs; } // devShell pkgs);
        })
    // {
      nixosModules.default = mkModule {
        inherit self pname description mainProgram format settingsDefaults secretFields extraServiceConfig;
      };
    };
}
