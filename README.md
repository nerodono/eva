# Eva

A backbone standard library for the VienDesu! projects. Carefully crafted functionality.

We emphasize:
- Correctness - the code inside is carefully crafted to ensure that it would be used in a correct way
- Performance - no perf trade-offs where possible
- Precise types - `eva` contains and allows you to define types more precisely, this ensures correctness statically and reveals more use-cases to the user and provides more information to the compiler

This library may re-export other crates selectively or entirely, but I strive to do that as rarely as possible.

## Flake library

The flake exports `lib` with shared Nix machinery for eva services: a pinned
Rust toolchain, crane-based package builds (git dependencies come straight from
`Cargo.lock`, no `outputHashes`), a dev shell, and a generated NixOS module
(`services.<pname>` with `settings`/`extraConfigFiles`/`user`/`group`/
`environment`, plus `secrets` with `secretFields`). A typical service flake is just:

```nix
{
  inputs.eva.url = "git+https://git.desu.church/VienDesu/eva.git";

  outputs = { self, eva, ... }: eva.lib.mkServiceFlake {
    inherit self;
    pname = "my-service";
    mainProgram = "my-service-server"; # default: pname
    description = "my-service, does things";
    settingsDefaults = { http.listen = "127.0.0.1:8080"; };
    secretFields = [ "db.password" ];
  };
}
```

Knobs: `version`, `env` (extra build env, e.g. an embedded commit hash),
`srcFilter` (extra non-cargo sources), `format` (`"toml"` default, `"yaml"`
with eva's `yaml` feature),
`extraServiceConfig` (merged into the systemd unit), `devShell`
(`pkgs: { stdenv, extraPackages, env }`). The pieces (`mkPkgs`,
`rustToolchain`, `mkPackage`, `mkDevShell`, `mkModule`) are exported
separately for nonstandard flakes.
