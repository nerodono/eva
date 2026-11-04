# Eva

Rust building blocks for VienDesu projects: data types, service interfaces, handlers,
and application startup.

```toml
[dependencies]
eva = { git = "https://github.com/nerodono/eva" }
```

```rust
use eva::{endpoint, service};

#[service]
pub trait Greetings {
    fn greet(name: String) -> Struct! { message: String };
}

#[endpoint(Greetings::greet)]
pub async fn Greet(_: (), args: this::Args) -> this::Output {
    Ok(this::Ok { message: format!("Hello, {}!", args.name) })
}
```

Use `eva` for the umbrella API, or depend on individual crates:

| Crate | Provides |
| --- | --- |
| [`eva-core`](core/) | Time, collections, hashing, utilities, and data reexports. |
| [`eva-data`](data/) | `#[data]`, `#[int]`, `#[str]`, their types, serialization, and schemas. |
| [`eva-handlers`](handlers/) | Endpoints, middleware, and service macros. |
| [`eva-rt`](rt/) | `Runtime`, `Components`, configuration, and executor setup. |

`eva` includes core and handlers by default. Enable `rt` for `eva::rt`, `cli` for
command-line parsing, and `logforth` for logging. Use `figment` for JSON/TOML and env
configuration, `yaml` for YAML, and `otel` for OpenTelemetry logging.

`eva-data/rand` adds random sampling for `#[int]` enums. `eva-core` enables it automatically.

Standalone `eva-handlers` has no dependencies by default; enable `macros` for
`#[service]`, `#[endpoint]`, and `#[export]`. Endpoints add tracing automatically;
`not(trace)` opts out, and `enable-tracing` enables span collection.

API docs: `cargo doc --open`.

## Nix

`eva.lib.mkServiceFlake` provides a pinned Rust toolchain, a crane build, a dev shell,
and a NixOS service module:

```nix
{
  inputs.eva.url = "github:nerodono/eva";

  outputs = { self, eva, ... }: eva.lib.mkServiceFlake {
    inherit self;
    pname = "my-service";
    mainProgram = "my-service-server"; # defaults to pname
    description = "My service";
    settingsDefaults = { http.listen = "127.0.0.1:8080"; };
    secretFields = [ "db.password" ];
  };
}
```

Import `nixosModules.default` and configure `services.my-service`: `enable`, `settings`,
`extraConfigFiles`, `user`, `group`, and `environment`. `secrets."db.password"` accepts
a secret file path delivered through systemd credentials. Config files use TOML by default;
`format = "yaml"` requires the Rust `yaml` feature.

Customize builds with `env`, `srcFilter`, and `devShell`; customize the unit with
`extraServiceConfig`. The individual helpers are also available in [`eva.lib`](nix/lib.nix).
