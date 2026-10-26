use crate::{
    data,
    fut::Fut,
    handling::{Apply, Handler},
};

#[data(copy, ord, crate = crate)]
pub struct Then<L, R> {
    pub lhs: L,
    pub rhs: R,
}

impl<I, S, L, R, N, Output> Handler<I, S, N> for Then<L, R>
where
    R: Send + Sync,
    L: for<'a> Handler<I, S, Apply<&'a R, N>, Output = Output>,
{
    type Output = Output;

    fn call(&self, state: S, in_: I, next: N) -> impl Fut<Output = Self::Output> {
        self.lhs.call(state, in_, Apply::new(&self.rhs, next))
    }
}

impl<L, R> Then<L, R> {
    pub const fn new(lhs: L, rhs: R) -> Self {
        Self { lhs, rhs }
    }
}
