use crate::CallStep;

/// A [`CallStep`] that calls a closure; see [`step`].
#[derive(Debug, Clone, Copy)]
pub struct FnStep<F>(pub F);

/// Turns `|args| async move { ... }` into a [`CallStep`], for hand-written service
/// implementations.
///
/// ```rust
/// use eva_handlers::{CallStep, step};
///
/// fn double() -> impl CallStep<u32, Output = u32> {
///     step(|x: u32| async move { x * 2 })
/// }
/// # fn main() { let _ = double(); }
/// ```
pub const fn step<F>(f: F) -> FnStep<F> {
    FnStep(f)
}

impl<I, F, R> CallStep<I> for FnStep<F>
where
    F: FnMut(I) -> R + Send + Sync,
    R: Future + Send,
{
    type Output = R::Output;

    fn call(&mut self, args: I) -> impl Future<Output = Self::Output> + Send {
        (self.0)(args)
    }
}
