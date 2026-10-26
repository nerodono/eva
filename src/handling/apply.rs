use crate::{
    data,
    fut::Fut,
    handling::{Endpoint, Handler},
};

#[data(copy, ord, crate = crate)]
pub struct Apply<L, R> {
    pub lhs: L,
    pub rhs: R,
}

impl<L, R> Apply<L, R> {
    pub const fn new(lhs: L, rhs: R) -> Self {
        Self { lhs, rhs }
    }
}

impl<I, S, L, R, Output> Endpoint<I, S> for Apply<L, R>
where
    L: for<'a> Handler<I, S, &'a R, Output = Output>,
    R: Send + Sync,
{
    type Output = Output;

    fn call(&self, state: S, in_: I) -> impl Fut<Output = Self::Output> {
        self.lhs.call(state, in_, &self.rhs)
    }
}
