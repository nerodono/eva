//! `#[endpoint]` behavior. The lints are denied so that a misrouted attribute fails the build:
//! the struct's docs satisfy `missing_docs`, and every `allow`/`expect` below must reach the
//! item where its lint fires.
#![deny(warnings, missing_docs)]

use std::{
    fmt::Debug,
    marker::PhantomData,
    pin::pin,
    str::FromStr,
    task::{Context, Poll, Waker},
};

use eva::{endpoint, handling::Endpoint};

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return out;
        }
    }
}

/// Adds the state to the args.
#[endpoint]
pub async fn Add(state: &u32, args: u32) -> u32 {
    *state + args
}

#[test]
fn call() {
    assert_eq!(block_on(Add.call(&2, 3)), 5);
}

#[test]
fn struct_derives() {
    fn assert_derives<T: Clone + Copy + Debug + Default>(_: T) {}
    assert_derives(Add);
    assert_eq!(format!("{Add:?}"), "Add");
}

/// Stand-in for a backend state that is generic over its components.
pub struct State<C> {
    hits: u32,
    _components: PhantomData<C>,
}

/// Counts hits in a mutably borrowed state with an elided lifetime.
#[endpoint(crate = ::eva)]
pub async fn Hit<C: Send + Sync>(state: &mut State<C>, args: u32) -> u32 {
    state.hits += args;
    state.hits
}

fn assert_any_borrow<C, E>(_: E)
where
    E: for<'a> Endpoint<u32, &'a mut State<C>, Output = u32>,
{
}

#[test]
fn generic_with_elided_lifetime() {
    assert_any_borrow::<(), _>(Hit);
    let mut state = State::<()> {
        hits: 1,
        _components: PhantomData,
    };
    assert_eq!(block_on(Hit.call(&mut state, 2)), 3);
    assert_eq!(state.hits, 3);
}

/// Every kind of generic parameter, each appearing in the args type.
#[endpoint]
pub async fn Pick<'a, T, const N: usize>(_state: (), args: (&'a [T; N], usize)) -> Option<&'a T>
where
    T: Sync,
{
    let (items, index) = args;
    items.get(index)
}

#[test]
fn generic_params() {
    assert_eq!(block_on(Pick.call((), (&[1, 2, 3], 1))), Some(&2));
}

/// Returns nothing.
#[endpoint]
pub async fn Unit(_state: (), _args: ()) {}

#[test]
fn default_return_type() {
    fn assert_output<E: Endpoint<(), (), Output = ()>>(_: E) {}
    assert_output(Unit);
    block_on(Unit.call((), ()));
}

/// Lint attributes reach both the struct and the impl, `inline` only `call`.
#[endpoint]
#[allow(non_camel_case_types, unused_variables)]
#[inline]
pub async fn snake_case(state: u32, args: u32) -> u32 {
    let unused = state;
    args
}

/// An expectation lands on the impl only, so it is fulfilled by the body alone.
#[endpoint]
#[expect(unused_variables)]
pub async fn Expect(state: u32, args: u32) -> u32 {
    args
}

/// Parameter attributes stay on the parameters.
#[endpoint]
pub async fn ParamAttrs(#[allow(unused_variables)] state: u32, args: u32) -> u32 {
    args
}

#[test]
fn lint_and_param_attrs() {
    assert_eq!(block_on(snake_case.call(1, 2)), 2);
    assert_eq!(block_on(Expect.call(1, 2)), 2);
    assert_eq!(block_on(ParamAttrs.call(1, 2)), 2);
}

/// Deprecated endpoints compile without warnings and warn where they are used.
#[endpoint]
#[deprecated = "use `Add`"]
pub async fn Old(_state: (), args: u32) -> u32 {
    args
}

#[test]
fn deprecated_struct() {
    #[expect(deprecated)]
    let old = Old;
    assert_eq!(block_on(old.call((), 4)), 4);
}

/// Generics reached through a qualified path's arguments and a trait object, next to a
/// projection of one of them.
#[endpoint]
pub async fn Mixed<T, U>(
    _state: (),
    args: (
        std::vec::Vec<T>,
        Option<T::Err>,
        Box<dyn Fn() -> U + Send + Sync>,
    ),
) -> (usize, U)
where
    T: FromStr + Send,
    T::Err: Send,
{
    let (items, err, make) = args;
    (items.len() + usize::from(err.is_some()), make())
}

#[test]
fn generics_beside_projections() {
    let args = (vec![1_u8, 2], None, Box::new(|| 'x') as Box<_>);
    assert_eq!(block_on(Mixed.call((), args)), (2, 'x'));
}

#[endpoint]
#[cfg_attr(all(), doc = "Documented through `cfg_attr`.")]
pub async fn CfgAttrDocs(_state: (), _args: ()) {}

#[endpoint]
#[cfg(any())]
pub async fn Gone(state: Undefined, args: ()) {}

#[test]
fn cfg_is_evaluated_first() {
    block_on(CfgAttrDocs.call((), ()));
}
