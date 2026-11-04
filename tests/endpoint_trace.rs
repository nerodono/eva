//! Tracing behavior of both endpoint forms, enabled with `--features enable-tracing`.
#![cfg(all(feature = "handlers", feature = "enable-tracing"))]
#![deny(warnings)]

use std::{
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use eva::{endpoint, handlers::Endpoint};
use fastrace::{
    Span,
    collector::{Config, Reporter, SpanContext, SpanRecord},
};

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

// Exercise suspension and resumption inside an instrumented endpoint.
async fn yield_once() {
    let mut yielded = false;
    std::future::poll_fn(|cx| {
        if std::mem::replace(&mut yielded, true) {
            Poll::Ready(())
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}

#[endpoint]
async fn Plain(_state: (), args: u32) -> u32 {
    yield_once().await;
    args + 1
}

#[eva::service]
pub trait Greetings {
    fn greet(name: String) -> Struct! { message: String };
}

#[endpoint(Greetings::greet)]
async fn Bound(_state: (), args: this::Args) -> this::Output {
    yield_once().await;
    Ok(this::Ok { message: args.name })
}

#[endpoint(not(trace))]
async fn Quiet(_state: (), _args: ()) {}

#[endpoint(Greetings::greet, not(trace))]
async fn QuietBound(_state: (), args: this::Args) -> this::Output {
    Ok(this::Ok { message: args.name })
}

#[endpoint]
#[fastrace::trace(name = "custom", properties = { "value": "{args}" })]
async fn Custom(_state: (), args: u32) -> u32 {
    args
}

#[endpoint(Greetings::greet)]
#[eva::_priv::fastrace::trace(crate = eva::_priv::fastrace, name = "custom-bound")]
async fn CustomBound(_state: (), args: this::Args) -> this::Output {
    Ok(this::Ok { message: args.name })
}

struct Records(Arc<Mutex<Vec<SpanRecord>>>);

impl Reporter for Records {
    fn report(&mut self, spans: Vec<SpanRecord>) {
        self.0.lock().unwrap().extend(spans);
    }
}

#[test]
fn traces_both_forms_and_respects_opt_out_and_explicit_attributes() {
    let records = Arc::new(Mutex::new(Vec::new()));
    fastrace::set_reporter(Records(records.clone()), Config::default());
    {
        let root = Span::root("test-root", SpanContext::random());
        let _parent = root.set_local_parent();
        assert_eq!(block_on(Plain.call((), 1)), 2);
        let args = || Greetings::greet::Args { name: "Eva".into() };
        assert_eq!(block_on(Bound.call((), args())).unwrap().message, "Eva");
        block_on(Quiet.call((), ()));
        block_on(QuietBound.call((), args())).unwrap();
        assert_eq!(block_on(Custom.call((), 42)), 42);
        block_on(CustomBound.call((), args())).unwrap();
    }
    fastrace::flush();
    let records = records.lock().unwrap();
    let mut names: Vec<_> = records.iter().map(|record| record.name.as_ref()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "Greetings::greet",
            "Plain",
            "custom",
            "custom-bound",
            "test-root"
        ]
    );
    let root = records
        .iter()
        .find(|record| record.name == "test-root")
        .unwrap();
    for record in records.iter().filter(|record| record.name != "test-root") {
        assert_eq!(record.parent_id, root.span_id);
    }
    let custom = records
        .iter()
        .find(|record| record.name == "custom")
        .unwrap();
    assert_eq!(custom.properties, [("value".into(), "42".into())]);
}
