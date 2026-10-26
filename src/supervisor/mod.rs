use compact_str::ToCompactString;
use eyre::eyre;
use fastrace::{Span, future::FutureExt, prelude::SpanContext};
use tokio::{sync::mpsc, task::JoinSet};

use crate::{
    collections::{HashMap, hash_map},
    component_configs::{ComponentConfig, ComponentConfigs, IsComponentConfig},
    data,
    fut::Fut,
    str::CompactString,
    sync::notifies::mpsc::{Tx, make as make_notify},
};

use self::scope::InScopeExt as _;

pub mod scope;

pub struct SlaveRx {
    rx: mpsc::Receiver<Command>,
}

impl SlaveRx {
    pub async fn recv(&mut self) -> Option<Command> {
        self.rx.recv().await
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

#[data(crate = crate)]
pub enum Command {
    Shutdown,
}

enum SupervisorCommand {
    Ping { pong: Tx },

    User(Command),
}

#[data(crate = crate, error)]
pub enum CommandError {
    #[display("the receiver is no longer alive")]
    Closed,
}

#[derive(Clone)]
pub struct Handle {
    tx: mpsc::Sender<SupervisorCommand>,
}

impl Handle {
    pub async fn request_shutdown(&self) -> CommandResult<()> {
        self.tx
            .send(SupervisorCommand::User(Command::Shutdown))
            .await
            .map_err(|_| CommandError::Closed)
    }

    pub async fn ping(&self) -> CommandResult<()> {
        let (tx, rx) = make_notify();

        self.tx
            .send(SupervisorCommand::Ping { pong: tx })
            .await
            .map_err(|_| CommandError::Closed)?;

        rx.await;

        Ok(())
    }
}

pub trait Supervised<T>:
    Send + Sync + 'static + Fn(ComponentConfig<T>, SlaveRx) -> Self::Fut
{
    type Fut: 'static + Fut<Output = eyre::Result<()>>;
}

impl<T, F, FFut> Supervised<T> for F
where
    F: Send + Sync + 'static + Fn(ComponentConfig<T>, SlaveRx) -> FFut,
    FFut: 'static + Fut<Output = eyre::Result<()>>,
{
    type Fut = FFut;
}

pub struct Supervisor {
    span: Span,
    set: JoinSet<eyre::Result<()>>,
    handles: HashMap<CompactString, Handle>,
    configs: ComponentConfigs,
}

impl Supervisor {
    pub fn new(configs: ComponentConfigs) -> Self {
        Self {
            span: Span::root("supervisor", SpanContext::random()),
            set: JoinSet::new(),
            handles: HashMap::default(),
            configs,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add<T>(mut self, f: impl Supervised<T>) -> Self
    where
        T: IsComponentConfig,
    {
        let (tx, mut rx) = mpsc::channel(8);
        let (stx, srx) = mpsc::channel(8);
        let handle = Handle { tx };
        let slave_rx = SlaveRx { rx: srx };

        let config = self.configs.get::<T>();
        let name = config.section().to_compact_string();

        let component_name = name.clone();
        self.set.spawn(
            async move {
                let scope = scope::Scope::new(&component_name);
                let fut = f(config, slave_rx).in_scope(scope.into());

                tokio::pin!(fut);
                loop {
                    tokio::select! {
                        res = &mut fut => {
                            break res;
                        }

                        cmd = rx.recv(), if !rx.is_closed() => {
                            let Some(cmd) = cmd else {
                                continue;
                            };

                            match cmd {
                                SupervisorCommand::Ping { pong } => {
                                    pong.wake();
                                }

                                SupervisorCommand::User(u) => {
                                    let is_shutdown = matches!(u, Command::Shutdown);
                                    if stx.send(u).await.is_err() {
                                        // The component dropped its SlaveRx, so it can't
                                        // drain by itself: cancel it instead of hanging.
                                        if is_shutdown {
                                            break Ok(());
                                        }
                                        rx.close();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            .in_span(Span::enter_with_parent(name.clone(), &self.span)),
        );

        let hash_map::Entry::Vacant(vac) = self.handles.entry(name.clone()) else {
            panic!("{name:?} already exists")
        };

        vac.insert(handle);

        self
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Handle)> + use<'_> {
        self.handles.iter().map(|(n, h)| (n.as_str(), h))
    }

    #[track_caller]
    pub fn handle(&self, h: &str) -> Handle {
        self.handles.get(h).expect("handle does not exist").clone()
    }

    /// Waits until every component exits.
    ///
    /// The first SIGINT/SIGTERM asks all components to shut down gracefully,
    /// the second one aborts them.
    pub async fn wait_for_completion(mut self) -> eyre::Result<()> {
        let handles: Vec<Handle> = self.handles.values().cloned().collect();

        async move {
            let mut signals = shutdown_signals();
            let mut signals_open = true;
            let mut draining = false;

            loop {
                tokio::select! {
                    res = self.set.join_next() => {
                        let Some(res) = res else {
                            break;
                        };

                        let error = match res {
                            Err(e) if e.is_cancelled() => None,
                            Err(e) => Some(eyre!("failed to join task: {e}")),
                            Ok(Err(e)) => Some(e),
                            Ok(Ok(())) => None,
                        };

                        if let Some(error) = error {
                            log::error!(error:%; "failed to join component");
                        }
                    }

                    signal = signals.recv(), if signals_open => {
                        let Some(signal) = signal else {
                            signals_open = false;
                            continue;
                        };

                        if draining {
                            log::warn!(signal; "second shutdown signal, aborting components");
                            self.set.abort_all();
                        } else {
                            log::info!(signal; "shutdown signal received, draining components");
                            draining = true;
                            for handle in &handles {
                                _ = handle.request_shutdown().await;
                            }
                        }
                    }
                }
            }
        }
        .in_scope(scope::Scope::new("system.supervisor").into())
        .await;

        fastrace::flush();

        Ok(())
    }
}

/// Yields the name of every received SIGINT/SIGTERM.
fn shutdown_signals() -> mpsc::Receiver<&'static str> {
    let (tx, rx) = mpsc::channel(2);

    tokio::spawn(async move {
        #[cfg(unix)]
        let mut term =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(term) => term,
                Err(error) => {
                    log::error!(error:%; "failed to listen for SIGTERM");
                    return;
                }
            };

        loop {
            #[cfg(unix)]
            let signal = tokio::select! {
                res = tokio::signal::ctrl_c() => res.map(|()| "SIGINT"),
                _ = term.recv() => Ok("SIGTERM"),
            };
            #[cfg(not(unix))]
            let signal = tokio::signal::ctrl_c().await.map(|()| "SIGINT");

            let signal = match signal {
                Ok(signal) => signal,
                Err(error) => {
                    log::error!(error:%; "failed to listen for SIGINT");
                    return;
                }
            };

            if tx.send(signal).await.is_err() {
                return;
            }
        }
    });

    rx
}
