mod filter;

use std::{collections::HashMap, path::PathBuf};

use logforth::layout::JsonLayout;

use crate::{component_configs::ComponentConfig, data, supervisor, utils::SecretString};

/// Diagnostic key carrying the active component name from
/// [`supervisor::scope`] into the log dispatcher.
pub(crate) const COMPONENT_DIAG_KEY: &str = "system_component";

/// Name reported by the async appender wrapper.
const ASYNC_APPENDER_NAME: &str = "system.logger";

#[data(crate = crate, copy, display(name))]
#[derive(Default)]
pub enum LogLevel {
    Debug,
    Warn,
    Error,
    #[default]
    Info,
    Trace,
    Disabled,
}

impl LogLevel {
    pub(crate) fn into_logforth(self) -> Option<logforth::record::Level> {
        use LogLevel::*;
        use logforth::record::Level as D;

        match self {
            Info => Some(D::Info),
            Warn => Some(D::Warn),
            Trace => Some(D::Trace),
            Error => Some(D::Error),
            Debug => Some(D::Debug),
            Disabled => None,
        }
    }
}

#[data(crate = crate)]
pub enum Append {
    File {
        base_dir: PathBuf,
        file_name: String,
    },
    /// OpenTelemetry log exporter. Only functional when the `otel`
    /// feature is enabled; without it, [`Append::try_to_appender`]
    /// reports a build error for this variant.
    Otel {
        name: String,
        endpoint: String,
        #[serde(default)]
        labels: HashMap<String, String>,
        basic_auth: Option<SecretString>,
    },
    Stderr,
    Stdout,
}

impl Append {
    pub fn try_to_appender(&self) -> eyre::Result<Box<dyn logforth::Append>> {
        use eyre::Context as _;
        use logforth::append;

        match self {
            Self::File {
                base_dir,
                file_name,
            } => {
                let file = append::file::FileBuilder::new(base_dir.clone(), file_name.clone())
                    .filename_suffix("log")
                    .layout(JsonLayout::default())
                    .build()
                    .wrap_err("failed to build file appender")?;
                Ok(Box::new(file))
            }
            Self::Otel {
                name,
                endpoint,
                labels,
                basic_auth,
            } => {
                #[cfg(feature = "otel")]
                {
                    Self::build_otel_appender(name, endpoint, labels, basic_auth.as_ref())
                }
                #[cfg(not(feature = "otel"))]
                {
                    let _ = (name, endpoint, labels, basic_auth);
                    Err(eyre::eyre!(
                        "Append::Otel requires the `otel` feature; rebuild eva with `--features otel`",
                    ))
                }
            }
            Self::Stderr => Ok(Box::new(append::Stderr::default())),
            Self::Stdout => Ok(Box::new(append::Stdout::default())),
        }
    }

    #[cfg(feature = "otel")]
    fn build_otel_appender(
        name: &str,
        endpoint: &str,
        labels: &HashMap<String, String>,
        basic_auth: Option<&SecretString>,
    ) -> eyre::Result<Box<dyn logforth::Append>> {
        use base64::Engine;
        use eyre::Context as _;
        use logforth::append;
        use opentelemetry_otlp::{WithExportConfig as _, WithHttpConfig};

        let mut headers = HashMap::default();
        if let Some(secret) = basic_auth {
            let creds = secret.read().wrap_err("failed to read basic_auth secret")?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
            headers.insert("Authorization".to_owned(), format!("Basic {encoded}"));
        }

        let exporter = opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_protocol(opentelemetry_otlp::Protocol::HttpJson)
            .with_headers(headers)
            .build()
            .wrap_err("failed to build otel log exporter")?;

        let appender =
            append::opentelemetry::OpentelemetryLogBuilder::new(name.to_owned(), exporter)
                .labels(labels.iter().map(|(k, v)| (k.clone(), v.clone())))
                .build();
        Ok(Box::new(appender))
    }
}

#[data(crate = crate)]
pub struct Config {
    #[serde(default = "Config::default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub default_level: LogLevel,
    #[serde(default)]
    pub components: HashMap<String, LogLevel>,
    #[serde(default)]
    pub targets: HashMap<String, LogLevel>,
    #[serde(default = "Config::default_append")]
    pub append: Vec<Append>,
}

impl Config {
    fn default_enabled() -> bool {
        true
    }

    fn default_append() -> Vec<Append> {
        vec![Append::Stderr]
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_level: LogLevel::default(),
            targets: HashMap::default(),
            enabled: Self::default_enabled(),
            components: HashMap::default(),
            append: Self::default_append(),
        }
    }
}

#[derive(Debug)]
struct ComponentDiagnostics {}

impl logforth::Diagnostic for ComponentDiagnostics {
    fn visit(&self, visitor: &mut dyn logforth::kv::Visitor) -> Result<(), logforth::Error> {
        use logforth::kv::{Key, Value};

        if let Some(r) = supervisor::scope::with(|scope| {
            visitor.visit(
                Key::new(COMPONENT_DIAG_KEY),
                Value::from_str(scope.component()),
            )
        }) {
            r
        } else {
            Ok(())
        }
    }
}

/// Initialize the global logger from a [`Config`].
///
/// All configured appenders are built before any global state is
/// touched. If any appender fails to build, the global logger is left
/// untouched and an aggregate error is returned listing every failure —
/// the caller is expected to abort startup. This is intentional: a
/// silent fallback would mask misconfigured sinks (e.g. an OTLP
/// endpoint pointing nowhere) in production.
pub fn setup(cfg: ComponentConfig<Config>) -> eyre::Result<()> {
    use std::fmt::Write as _;

    use logforth::{
        append::{self, asynchronous as async_},
        diagnostic,
    };

    let mut appenders: Vec<Box<dyn logforth::Append>> = Vec::with_capacity(cfg.append.len());
    let mut errors: Vec<(String, eyre::Report)> = Vec::new();

    for spec in cfg.append.iter() {
        match spec.try_to_appender() {
            Ok(a) => appenders.push(a),
            Err(e) => errors.push((format!("{spec:?}"), e)),
        }
    }

    if !errors.is_empty() {
        let mut msg = String::from("failed to initialize log appender(s):");
        for (name, err) in &errors {
            let _ = write!(&mut msg, "\n  - {name}: {err:#}");
        }
        return Err(eyre::eyre!("{msg}"));
    }

    logforth::starter_log::builder()
        .dispatch(move |d| {
            let mut async_builder = async_::AsyncBuilder::new(ASYNC_APPENDER_NAME);
            for appender in appenders {
                async_builder = async_builder.append(appender);
            }

            d.diagnostic(ComponentDiagnostics {})
                .filter(filter::LogFilter { cfg })
                .diagnostic(diagnostic::FastraceDiagnostic::default())
                .append(append::FastraceEvent::default())
                .append(async_builder.build())
        })
        .apply();

    // Default reporter, so collected spans are exported somewhere;
    // with tracing disabled fastrace is a zero-cost no-op.
    #[cfg(feature = "enable-tracing")]
    fastrace::set_reporter(
        fastrace::collector::ConsoleReporter,
        fastrace::collector::Config::default(),
    );

    Ok(())
}
