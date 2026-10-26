use crate::{auto_impl, fut::Fut};

pub use self::{apply::Apply, then::Then};

pub trait CallStep<I>: Send + Sync {
    type Output;

    fn call(&mut self, args: I) -> impl Fut<Output = Self::Output>;
}

mod apply;
mod then;

pub trait HandlerExt: Sized {
    fn then<R>(self, rhs: R) -> Then<Self, R> {
        Then::new(self, rhs)
    }
}

impl<T> HandlerExt for T {}

pub trait EndpointExt: Sized {
    fn apply<R>(self, rhs: R) -> Apply<Self, R> {
        Apply::new(self, rhs)
    }
}

impl<T> EndpointExt for T {}

#[auto_impl(&, &mut, Arc, Box)]
pub trait Endpoint<I, S>: Send + Sync {
    type Output;

    fn call(&self, state: S, in_: I) -> impl Fut<Output = Self::Output>;
}

#[auto_impl(&, &mut, Arc, Box)]
pub trait Handler<I, S, N>: Send + Sync {
    type Output;

    fn call(&self, state: S, in_: I, next: N) -> impl Fut<Output = Self::Output>;
}
