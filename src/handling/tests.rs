use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use crate::handling::{CallStep, step};

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
