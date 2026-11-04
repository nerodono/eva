use eva_core::data;
use eva_rt::{
    Components,
    cfg::{Builder, ComponentConfig},
    components::{Command, SlaveRx},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[data]
struct Settings {
    message: String,
}

#[tokio::test]
async fn programmatic_component_shutdown() -> eyre::Result<()> {
    let stopped = Arc::new(AtomicBool::new(false));
    let marker = stopped.clone();
    let configs = Builder::default()
        .set(
            "worker",
            Settings {
                message: "configured".into(),
            },
        )
        .build()?;
    let components = Components::new(configs).add(
        move |cfg: ComponentConfig<Settings>, mut commands: SlaveRx| {
            let marker = marker.clone();
            async move {
                assert_eq!(cfg.message, "configured");
                assert_eq!(
                    eva_rt::components::scope::with(|scope| scope.component().to_owned()),
                    Some("worker".into())
                );
                assert!(matches!(commands.recv().await, Some(Command::Shutdown)));
                marker.store(true, Ordering::SeqCst);
                Ok(())
            }
        },
    );
    let handle = components.handle("worker");
    handle.ping().await?;
    handle.request_shutdown().await?;
    components.wait_for_completion().await?;
    assert!(stopped.load(Ordering::SeqCst));
    Ok(())
}
