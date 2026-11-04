use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::{CallStep, step};
use crate::{Endpoint, EndpointExt, Handler, HandlerExt};
use std::sync::Arc;

fn run<F: Future>(future: F) -> F::Output {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test step must be immediately ready"),
    }
}

#[test]
fn fn_step_keeps_closure_state_between_calls() {
    let mut calls = 0;
    let mut bound = step(move |x: u32| {
        calls += 1;
        async move { x + calls }
    });
    assert_eq!(run(bound.call(1)), 2);
    assert_eq!(run(bound.call(1)), 3);
}

struct Echo;

impl Endpoint<u32, u32> for Echo {
    type Output = u32;

    async fn call(&self, state: u32, args: u32) -> u32 {
        state + args
    }
}

struct Add(u32);

impl<N: Endpoint<u32, u32, Output = u32>> Handler<u32, u32, N> for Add {
    type Output = u32;

    async fn call(&self, state: u32, args: u32, next: N) -> u32 {
        next.call(state, args + self.0).await
    }
}

#[test]
fn forwarding_and_composition_work_without_proc_macros() {
    fn endpoint(endpoint: impl Endpoint<u32, u32, Output = u32>) {
        assert_eq!(run(endpoint.call(10, 2)), 12);
    }
    fn handler(handler: impl Handler<u32, u32, Echo, Output = u32>) {
        assert_eq!(run(handler.call(10, 2, Echo)), 15);
    }
    let mut echo = Echo;
    endpoint(&echo);
    endpoint(&mut echo);
    endpoint(Box::new(Echo));
    endpoint(Arc::new(Echo));

    let mut add = Add(3);
    handler(&add);
    handler(&mut add);
    handler(Box::new(Add(3)));
    handler(Arc::new(Add(3)));

    let chain = Add(3).then(Add(5)).apply(Echo);
    assert_eq!(run(chain.call(10, 2)), 20);
}
