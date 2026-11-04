//! Endpoints, middleware, and composable calls. No dependencies without features.
extern crate self as eva_handlers;

#[cfg(feature = "macros")]
pub use eva_data::data;
#[cfg(feature = "macros")]
pub use eva_handlers_macros::{endpoint, export, if_flag, service};

#[cfg(feature = "macros")]
#[doc(hidden)]
pub mod _priv {
    pub use crate::{CallStep, Endpoint};
    pub use eva_data::_priv::*;
    pub use eva_handlers_macros::check_flags;
    pub use fastrace;
}

pub use self::{
    apply::Apply,
    fn_step::{FnStep, step},
    then::Then,
};

/// A call bound to its state. Service traits return one per method: the binding wraps
/// the endpoint in [`Handler`] middleware with `then`/`apply` and binds it to state.
pub trait CallStep<I>: Send + Sync {
    type Output;

    fn call(&mut self, args: I) -> impl Future<Output = Self::Output> + Send;
}

mod apply;
mod fn_step;
mod then;

#[cfg(test)]
mod tests;

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

pub trait Endpoint<I, S>: Send + Sync {
    type Output;

    fn call(&self, state: S, in_: I) -> impl Future<Output = Self::Output> + Send;
}

pub trait Handler<I, S, N>: Send + Sync {
    type Output;

    fn call(&self, state: S, in_: I, next: N) -> impl Future<Output = Self::Output> + Send;
}

// These two fixed traits need forwarding even without the proc-macro feature.
macro_rules! delegate {
    ($inner:ident; $($wrapper:ty),+ $(,)?) => {$(
        #[automatically_derived]
        impl<I, S, $inner: ?Sized + Endpoint<I, S>> Endpoint<I, S> for $wrapper {
            type Output = $inner::Output;
            fn call(&self, state: S, args: I) -> impl Future<Output = Self::Output> + Send {
                <$inner as Endpoint<I, S>>::call(&**self, state, args)
            }
        }
        #[automatically_derived]
        impl<I, S, N, $inner: ?Sized + Handler<I, S, N>> Handler<I, S, N> for $wrapper {
            type Output = $inner::Output;
            fn call(&self, state: S, args: I, next: N) -> impl Future<Output = Self::Output> + Send {
                <$inner as Handler<I, S, N>>::call(&**self, state, args, next)
            }
        }
    )+};
}

delegate!(T; &T, &mut T, Box<T>, std::sync::Arc<T>);
