use eva_core::data;
use eva_rt::cfg::Builder;

#[data]
#[derive(Default)]
struct Settings {
    count: u32,
}

#[test]
fn typed_config_defaults_required_values_and_overrides() {
    let default = Builder::default()
        .optional::<Settings>("worker")
        .build()
        .unwrap();
    assert_eq!(default.get::<Settings>().count, 0);
    assert_eq!(default.get::<Settings>().section(), "worker");
    let mut config = default.get::<Settings>();
    assert!(!config.outdated());
    assert!(config.try_update().is_none());

    assert!(
        Builder::default()
            .require::<Settings>("worker")
            .build()
            .is_err()
    );
    let supplied = Builder::default()
        .require::<Settings>("worker")
        .set("worker", Settings { count: 7 })
        .build()
        .unwrap();
    assert_eq!(supplied.get::<Settings>().count, 7);
}

#[cfg(feature = "figment")]
#[test]
fn providers_override_in_order_and_invalid_sections_do_not_use_defaults() {
    use figment::providers::{Format, Toml};
    let configs = Builder::default()
        .optional::<Settings>("worker")
        .merge(Toml::string("[worker]\ncount = 2"))
        .merge(Toml::string("[worker]\ncount = 9"))
        .build()
        .unwrap();
    assert_eq!(configs.get::<Settings>().count, 9);
    assert!(
        Builder::default()
            .optional::<Settings>("worker")
            .merge(Toml::string("[worker]\ncount = 'invalid'"))
            .build()
            .is_err()
    );
}

#[cfg(feature = "figment")]
fn config_file(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[cfg(feature = "figment")]
#[test]
fn files_merge_in_order() {
    let configs = Builder::default()
        .require::<Settings>("worker")
        .file(config_file("config.toml"))
        .unwrap()
        .file(config_file("config.json"))
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(configs.get::<Settings>().count, 7);

    let yaml = Builder::default()
        .require::<Settings>("worker")
        .file(config_file("config.yaml"));
    #[cfg(feature = "yaml")]
    assert_eq!(yaml.unwrap().build().unwrap().get::<Settings>().count, 9);
    #[cfg(not(feature = "yaml"))]
    assert!(yaml.is_err());
}

#[cfg(feature = "figment")]
#[test]
fn environment_overrides_files_but_not_typed_values() {
    // A subprocess keeps environment mutation out of the multithreaded test runner.
    if std::env::var_os("EVA_TEST_CFG_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "environment_overrides_files_but_not_typed_values",
            ])
            .env("EVA_TEST_CFG_CHILD", "1")
            .env("EVA_TEST_CFG_WORKER__COUNT", "11")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }

    let builder = || {
        Builder::default()
            .require::<Settings>("worker")
            .env_prefix("EVA_TEST_CFG_")
            .file(config_file("config.toml"))
            .unwrap()
    };
    assert_eq!(builder().build().unwrap().get::<Settings>().count, 11);
    let typed = builder().set("worker", Settings { count: 42 });
    assert_eq!(typed.build().unwrap().get::<Settings>().count, 42);
}

#[test]
fn executor_builds_both_flavors() {
    use eva_rt::executor::{Config, Flavor, ThreadsCount};
    let single = Config {
        flavor: Flavor::SingleThreaded,
    }
    .build()
    .unwrap();
    single.block_on(async {
        assert_eq!(tokio::spawn(async { 42 }).await.unwrap(), 42);
    });
    let multi = Config {
        flavor: Flavor::MultiThreaded {
            threads: ThreadsCount::Specific(2.try_into().unwrap()),
        },
    }
    .build()
    .unwrap();
    multi.block_on(async {
        assert_eq!(tokio::spawn(async { 7 }).await.unwrap(), 7);
    });
}

#[cfg(feature = "cli")]
#[test]
fn cli_can_parse_without_starting_an_executor() {
    #[derive(Debug, clap::Args)]
    struct Args {
        #[arg(long)]
        count: u32,
    }
    let args = eva_rt::cli::Args::<Args>::try_from_iter([
        "app",
        "--count",
        "12",
        "-c",
        "first.toml",
        "--config",
        "second.json",
    ])
    .unwrap();
    assert_eq!(args.args.count, 12);
    assert_eq!(
        args.config,
        [
            std::path::PathBuf::from("first.toml"),
            std::path::PathBuf::from("second.json")
        ],
    );
    assert!(eva_rt::cli::Args::<Args>::try_from_iter(["app", "--count", "bad"]).is_err());
}
