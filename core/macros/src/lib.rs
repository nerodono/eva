use eva_macros_common::auto_impl;
use proc_macro::TokenStream;

/// Implements the trait for `&T`, `&mut T`, `Box<T>`, `Rc<T>` and `Arc<T>` by forwarding to `T`.
///
/// ```rust
/// #[eva::auto_impl(&, &mut, Box)]
/// trait Counter {
///     const STEP: u32;
///     type Out;
///     fn get(&self) -> Self::Out;
/// }
///
/// struct Fixed;
/// impl Counter for Fixed {
///     const STEP: u32 = 1;
///     type Out = u32;
///     fn get(&self) -> u32 { 7 }
/// }
///
/// fn read(c: impl Counter<Out = u32>) -> u32 { c.get() }
/// assert_eq!(read(&Fixed) + read(Box::new(Fixed)), 14);
/// ```
///
/// Allowed receivers per proxy: `&` and `Rc`/`Arc` take `&self` only, `&mut` adds `&mut self`,
/// `Box` adds `self`; methods without a receiver forward everywhere. `T: ?Sized` unless a
/// method takes or returns `Self` by value or requires `Self: Sized`; method-level
/// `where Self: Trait` bounds become bounds on `T`. Arguments must be plain names.
#[proc_macro_attribute]
pub fn auto_impl(args: TokenStream, item: TokenStream) -> TokenStream {
    eva_macros_common::run(args.into(), item.into(), auto_impl::expand, |item| item).into()
}
