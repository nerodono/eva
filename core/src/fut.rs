use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// Future which is never ready.
#[crate::perfect_derive(Debug, Clone, Default, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Never<T>(PhantomData<T>);

impl<T> Never<T> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Future for Never<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

crate::trait_set! {
    /// Future + Send.
    pub trait Fut = Future + Send;
}
