use eva_macros_common::{docs, opts, utils};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

fn run(
    args: TokenStream,
    item: TokenStream,
    expand: eva_macros_common::Expand,
    fallback: fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> TokenStream {
    eva_macros_common::run(args.into(), item.into(), expand, fallback).into()
}
mod endpoint;
mod export;
mod flags;
mod service;

/// Turns an async fn into a unit struct that implements `eva_handlers::Endpoint`
/// (also available as `eva::handlers::Endpoint`).
///
/// ```text
/// #[endpoint]
/// [attrs] vis async fn Name<generics>(state: S, args: A) -> O where ... { body }
///
/// #[endpoint(Service::method)]
/// [attrs] vis async fn Name(state: S, args: this::Args) -> this::Output { body }
/// ```
///
/// expands to
///
/// ```text
/// [struct attrs]
/// #[derive(Clone, Copy, Debug, Default)]
/// vis struct Name;
///
/// [impl attrs]
/// impl<generics> Endpoint<A, S> for Name where ... {
///     type Output = O;
///
///     #[fastrace::trace(name = "Name")]
///     [call attrs]
///     async fn call(&self, state: S, args: A) -> Self::Output { body }
/// }
/// ```
///
/// `Name` is a value: pass it wherever an `Endpoint` is expected and run it with
/// `Name.call(state, args).await`. There is no fn left to call as `Name(state, args)`. The struct
/// takes the fn's name, so name endpoints in CamelCase.
///
/// ## Options
///
/// | Option | Meaning |
/// | --- | --- |
/// | `Service::method` | Optional first argument: expose the method's types and a local `Output` through `this`. |
/// | `not(trace)` | Disable automatic tracing for this endpoint. |
/// | `crate = path` | Override the runtime path; otherwise discover `eva-handlers`, then `eva`, respecting Cargo dependency renames. |
///
/// Options follow the module path, when present: `#[endpoint(Service::method, not(trace))]`.
/// The plain `#[endpoint]` form continues to accept arbitrary input and output types.
///
/// ## Method scope
///
/// `#[endpoint(Service::method)]` creates a local `this` module in an anonymous const around
/// the impl. It re-exports the method's types and adds
/// `type Output = Service::Result<Ok, Err>`, with the success type's lifetimes when needed.
/// `Output` belongs to this endpoint scope; the service's method module has no such alias.
/// Each endpoint gets its own namespace, visible in its signature and body; the impl stays
/// in the surrounding module, preserving its imports and `self`/`super` paths.
/// The module path takes no generic arguments; type arguments go on the types, such as
/// `this::Args<'a>` and `this::Output<'a>`. Explicit types may still be used with this form.
/// Re-exported methods and imported method aliases retain their service's result constructor.
///
/// `this::Ok { field: value }`, `this::Ok(value)`, and `this::Err::Variant(value)` are ordinary
/// Rust constructors. Enum variants and patterns work without helper macros. Other items
/// in the method module, including those defined by `embed!`, are also available through `this`.
///
/// ## Tracing
///
/// Both forms add a fastrace span by default. Its name is the module path as written in the
/// attribute (`Service::method`), or the original fn name (`Name`) without a module binding.
/// The generated attribute reaches fastrace through the selected runtime crate;
/// no direct fastrace dependency is needed for automatic tracing.
///
/// `fastrace/enable` controls collection; `eva-handlers` or `eva`'s `enable-tracing` feature enables it. Without
/// that feature on fastrace, its attribute leaves the function uninstrumented. Collecting spans
/// requires a reporter and a parent tracing context supplied by the caller.
///
/// A qualified `#[fastrace::trace(...)]` placed below `#[endpoint]` replaces the automatic
/// attribute, preserving its custom name and properties without adding a second span.
/// Qualified runtime reexports and Cargo dependency renames are recognized too. A local
/// `use` alias cannot be resolved; use `not(trace)` when adding an attribute through one.
/// `not(trace)` only disables the automatic attribute,
/// leaving explicit attributes in place.
///
/// ## Signature rules
///
/// Violations are compile errors.
///
/// - The fn is `async`, and not `const`, `unsafe` (`Endpoint::call` is safe), `extern` or variadic.
/// - It takes exactly two typed parameters, state first, then args; any patterns, no `self`.
/// - No `impl Trait` in parameter types (use a generic parameter) or in the return type (use a
///   concrete type).
/// - Every generic parameter (type, lifetime or const) appears in `S` or `A`, otherwise the impl
///   cannot constrain it. A projection such as `T::Assoc` or `<T as Trait>::Assoc` does not count.
/// - Elided lifetimes in `S` and `A` work (`state: &mut State<C>` implements the endpoint for every
///   borrow). A lifetime in `O` must be a named one taken from `S` or `A`.
///
/// ## Attributes
///
/// | On the fn                                              | Goes to                 |
/// |--------------------------------------------------------|-------------------------|
/// | doc comments, `#[doc(...)]`, `#[deprecated]`           | the struct              |
/// | `#[allow]`, `#[warn]`, `#[deny]`, `#[forbid]`          | the struct and the impl |
/// | `#[expect]`                                            | the impl                |
/// | unsafe attributes, `#[must_use]`                       | rejected                |
/// | anything else (`#[fastrace::trace]`, `#[inline]`, ...) | `call`                  |
///
/// Unsafe attributes (`#[unsafe(...)]` and the bare `#[no_mangle]`, `#[export_name]`,
/// `#[link_section]`) and `#[must_use]` are rejected: on `call` they would not do what they
/// promise for the fn.
///
/// `#[expect]` is not copied to the struct: that copy would be unfulfilled whenever the lint fires
/// only in the body. Lints of the struct itself, such as `non_camel_case_types`, take `#[allow]`.
/// Parameter attributes stay on their parameters. `#[cfg]` and `#[cfg_attr]` are evaluated before the
/// macro runs: `#[cfg]` gates the whole expansion and a `cfg_attr`'s result is routed like any other
/// attribute. The impl of a deprecated endpoint does not silence deprecation lints in the body. A body
/// whose future is not `Send` is reported at the return type (at the name when there is none).
///
/// ## Examples
///
/// ```rust
/// use eva::{endpoint, handlers::Endpoint};
///
/// /// Adds the state to the args.
/// #[endpoint]
/// pub async fn Add(state: &u32, args: u32) -> u32 {
///     *state + args
/// }
///
/// # fn block_on<F: Future>(fut: F) -> F::Output {
/// #     let mut fut = std::pin::pin!(fut);
/// #     let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
/// #     loop {
/// #         if let std::task::Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
/// #             return out;
/// #         }
/// #     }
/// # }
/// assert_eq!(block_on(Add.call(&2, 3)), 5);
/// ```
///
/// An endpoint bound to a service method, with automatic tracing and local type names:
///
/// ```rust
/// #[eva::service]
/// pub trait Greetings {
///     fn greet(name: String) -> Struct! { message: String };
/// }
///
/// pub struct State { pub prefix: String }
///
/// #[eva::endpoint(Greetings::greet)]
/// pub async fn Greet(state: &State, args: this::Args) -> this::Output {
///     Ok(this::Ok {
///         message: format!("{}, {}!", state.prefix, args.name),
///     })
/// }
/// # fn main() {}
/// ```
///
/// A generic endpoint over a mutably borrowed state, with `crate = path` and routed attributes:
///
/// ```rust
/// use eva::handlers::Endpoint;
///
/// pub struct State<C> {
///     pub hits: u32,
///     pub config: C,
/// }
///
/// /// Counts hits.
/// #[eva::endpoint(crate = ::eva)]
/// #[deprecated = "count hits elsewhere"]
/// #[inline]
/// pub async fn Hit<C: Send + Sync>(state: &mut State<C>, args: u32) -> u32 {
///     state.hits += args;
///     state.hits
/// }
///
/// fn assert_endpoint<C>(_: impl for<'a> Endpoint<u32, &'a mut State<C>, Output = u32>) {}
/// #[expect(deprecated)]
/// let hit = Hit;
/// assert_endpoint::<()>(hit);
/// ```
///
/// A generic parameter that only the return type mentions is rejected:
///
/// ```rust,compile_fail
/// #[eva::endpoint]
/// pub async fn Make<T: Default>(state: (), args: ()) -> T {
///     T::default()
/// }
/// ```
#[proc_macro_attribute]
pub fn endpoint(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, endpoint::expand, endpoint::fallback)
}

/// Defines a service and its endpoint metadata from a trait declaration.
///
/// The runtime path is discovered from `eva-handlers`, then `eva`, respecting Cargo
/// dependency renames. `#[service(crate = path)]` overrides discovery.
///
/// Write the annotated declaration directly in source. Generating it through `macro_rules!`
/// is unsupported: exported helper names use source coordinates, which may coincide across
/// expansions. This restriction can be lifted when macros 2.0 provides public module-scoped
/// helpers without `#[macro_export]`.
///
/// ```rust
/// use eva::{export, service};
///
/// #[eva::data(error, display("user not found"))]
/// pub struct NotFound;
///
/// #[service]
/// pub trait Users {
///     #[errors(NoSuchUser(NotFound))]
///     fn get(id: u64) -> String;
///
///     #[flags(write)]
///     fn create(name: String) -> Struct! { id: u64 };
/// }
///
/// #[export(flags(write))]
/// pub struct Session {
///     users: Users,
/// }
/// # fn main() {}
/// ```
///
/// The name is preserved exactly: `Users` becomes a module containing:
///
/// - `Result<T, E>`, the service's configured result container.
/// - `Trait`, with `fn get(&mut self) -> impl CallStep<get::Args, Output = Result<get::Ok, get::Err>>`.
///   The trait is also implemented for `&mut S` and `Box<S>`.
/// - `Client<S>`, which owns any `S: Trait`. Its methods accept the declared arguments,
///   construct `Args`, and return a `Send` future. `Client::new(backend)`,
///   `Client::new(&mut backend)` and `Client::new(Box::new(backend))` are supported.
///   `inner_mut()` exposes the underlying service trait for callers that already have `Args`.
/// - One module per method, containing `Args`, `Ok`, `Err` and any embedded items.
/// - `for_each_endpoint!`, the service metadata callback used by `#[export]`.
///
/// `#[service(export)]` also re-exports every method module beside the service.
/// `#[service(export(get, create))]` re-exports only the selected methods. The
/// re-exports have the same visibility as the service; `Trait` and `Client` stay
/// in the service module.
///
/// `-> T` creates `pub struct Ok(pub T)` with `From<T>` and transparent
/// serialization. To customize that wrapper, put `#[ok(...)]` on the method,
/// for example `#[ok(data(copy))] fn id() -> u64;`. Omitted return types mean `()`.
/// `-> Struct! { field: T }` creates a success structure with public named fields;
/// `-> Enum! { Record { field: T }, Tuple(T), Simple }` creates a success enum.
/// Both forms define the method's `Ok` type and accept ordinary Rust field and variant
/// attributes. Inner attributes at the beginning apply to the generated type:
///
/// ```rust
/// use eva::service;
///
/// #[service]
/// pub trait Health {
///     fn status() -> Struct! {
///         #![data(copy)]
///         #![derive(Default)]
///
///         ready: bool,
///     };
///
///     fn type_of_num(num: i8) -> Enum! {
///         #![data(copy)]
///         #![serde(rename_all = "snake_case")]
///
///         Even,
///         Odd,
///     };
/// }
/// # fn main() {}
/// ```
///
/// Arguments always become named fields, including a single argument. Empty argument
/// lists produce `Args {}`. Lifetimes in argument types are carried into `Args` and
/// client methods; elided reference lifetimes and `'_` are inferred.
///
/// `#[errors(path::Error, OtherError(other::Error))]` creates a per-method enum with
/// payloads, `From` implementations and delegated error display. Repeated variant
/// names require an explicit `Variant(path::Error)` name. No errors attribute means an empty enum.
/// `#[service(error = path::Error)]` selects one shared error type for all methods;
/// `#[error(path::Error)]` selects an existing error type for one method.
///
/// `#[flags(write, stream)]` tags a method for metadata consumers; the session's
/// `#[export(flags(...))]` declares which flags exist.
///
/// `Result<T, E>` defaults to `::core::result::Result<T, E>`.
/// `#[service(result = path::Response)]` defines it as `path::Response<T, E>` instead,
/// for example an alias for `Result<T, Generic<E>>`. The trait, client and metadata express
/// each method's result as `Service::Result<method::Ok, method::Err>` directly; method
/// modules do not define an `Output` alias. A bound [`endpoint`] provides `this::Output`
/// locally for its implementation.
///
/// Generated data uses `#[data]` through the selected runtime. `#[args(...)]`, `#[ok(...)]` and `#[err(...)]`
/// forward attributes to the corresponding type; inner attributes inside `Struct!`
/// and `Enum!` also customize `Ok`.
/// A supplied `data(...)` replaces its default configuration, except that `Err` always
/// keeps `error`. A stream argument can use
/// `#[args(data(not(Debug, serde, schemars, Clone, PartialEq)))]`. Argument and
/// response field attributes are preserved, including serde defaults and documentation.
/// Explicit lifetimes in response fields are carried into `Ok` and must occur in an
/// argument type, for both structures and enums.
///
/// `embed! { ... }` places ordinary Rust items in the preceding method's module.
/// It must follow a method; whitespace does not affect this association.
/// In method declarations and embedded items, `this::X` refers to `X` in that method's
/// module. Each method module and client method gets its own ordinary `this` import;
/// generic arguments, nested paths and uses inside other macros follow Rust's normal
/// name resolution. `this` is reserved for this import. Nested embedded modules can
/// import it with `use super::this;`. `Struct!`, `Enum!` and `embed!` are consumed by
/// `service` and need no imports. Inside the method module `Ok` and `Err`
/// name the generated types, so embedded code uses qualified result constructors, such as
/// `::core::result::Result::Ok` and `::core::result::Result::Err` for the standard container.
///
/// ```rust
/// use eva::service;
///
/// #[service(export)]
/// pub trait Values {
///     fn get(value: this::Value<i32>) -> Struct! { value: this::Value<i32> };
///     embed! {
///         #[eva::data]
///         pub struct Value<T>(pub T);
///     }
/// }
/// # fn main() {}
/// ```
///
/// Each `Trait` method returns a `CallStep`, and that is where the binding attaches
/// middleware: it wraps the endpoint in `Handler`s, e.g.
/// `LogCall("users.get").then(Auth).apply(handlers::Get)`, and binds the result to state.
/// Service declarations and client calls need no changes. `eva::handlers::step` turns a
/// closure into a `CallStep` for hand-written implementations.
#[proc_macro_attribute]
pub fn service(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, service::expand, |_| TokenStream2::new())
}

/// Composes service modules into a session and a single X-macro.
///
/// The runtime path is discovered from `eva-handlers`, then `eva`, respecting Cargo
/// dependency renames. `#[export(crate = path)]` overrides discovery.
///
/// Write the annotated declaration directly in source. As with [`service`], generating it
/// through `macro_rules!` is unsupported because exported helpers use source coordinates
/// for their names. Metadata callbacks can still be ordinary `macro_rules!` macros.
///
/// ```rust
/// use eva::{export, service};
///
/// #[service]
/// pub trait Health {
///     fn ping() -> eva::True;
/// }
///
/// #[export]
/// pub struct Session {
///     health: Health,
/// }
/// # fn main() {}
/// ```
///
/// The fields declare accessors and service module paths. They are not stored fields:
/// the generated `Session<S>(pub S)` stores one implementation. `Session::new`,
/// `into_inner`, `inner_mut`, and `mut_` construct, extract or borrow it. Each accessor
/// returns the service's `Client<&mut S>`. `SessionFeatures` is the combined service
/// bound and has a blanket implementation.
///
/// The macro `for_each_endpoint!` calls its callback once with all services. Use
/// `#[export(name = for_each_op)]` to choose a different name. If the session is
/// declared in a nested module, re-export the macro at the crate root under the
/// same name. Service paths resolve where the session is declared, so ordinary
/// private imports and aliases work.
/// The macro has a hidden companion module of the same name; the same `pub use`
/// re-exports both, keeping metadata paths valid across crates and dependency renames.
///
/// `#[export(flags(write, stream))]` declares the method flags the services may use;
/// any other flag is a compile error at its `#[flags(...)]`.
///
/// ```ignore
/// macro_rules! project {
///     ($(
///         $accessor:ident {
///             trait: $Trait:path,
///             client: $Client:path,
///             methods: {$(
///                 $method:ident {
///                     flags: [$($flag:ident),*],
///                     lifetimes: [$($lt:lifetime),*],
///                     args: $Args:ty,
///                     ok: $Ok:ty,
///                     err: $Err:ty,
///                     output: $Output:ty,
///                     docs: [$($doc:literal),*],
///                     $($method_extra:tt)*
///                 }
///             )*},
///             $($extra:tt)*
///         }
///     )*) => { /* implementations, transport routes, tools, etc. */ };
/// }
/// for_each_endpoint!(project);
/// ```
///
/// The trailing `tt` captures keep callbacks matching when the metadata gains fields.
/// `fn $method<$($lt),*>(&mut self)` accepts an empty lifetime list. Test flags with
/// [`if_flag!`](macro@if_flag). Trait and client paths are provided directly; callbacks do
/// not have to reconstruct them. Method flags and documentation come from `#[service]`,
/// so no second method registry is needed.
/// The `output` field is a composed `Service::Result<method::Ok, method::Err>` type;
/// capture it as `ty` rather than assuming a method-local type alias.
///
/// A backend can select middleware in its callback using a separate expression:
///
/// ```ignore
/// macro_rules! decorate {
///     (users, get; $handler:expr) => {
///         LogCall("users.get").apply($handler)
///     };
///     ($service:ident, $method:ident; $handler:expr) => { $handler };
/// }
/// ```
#[proc_macro_attribute]
pub fn export(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, export::expand, |_| TokenStream2::new())
}

/// Expands to the first block when the flag is in the list, else to the `else` block, if any.
///
/// ```rust
/// macro_rules! op {
///     ([$($flag:ident),*]) => {
///         eva::if_flag!(write in [$($flag),*] { "write" } else { "read" })
///     };
/// }
/// assert_eq!(op!([write, stream]), "write");
/// assert_eq!(op!([]), "read");
/// ```
///
/// The chosen block's contents expand in place, so it works in item, statement and
/// expression position.
#[proc_macro]
pub fn if_flag(input: TokenStream) -> TokenStream {
    flags::if_flag(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[doc(hidden)]
#[proc_macro]
pub fn check_flags(input: TokenStream) -> TokenStream {
    flags::check(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn runtime() -> syn::Path {
    eva_macros_common::resolve_crates(&["eva-handlers", "eva"])
        .next()
        .map(|(_, path)| path)
        .unwrap_or_else(|| syn::parse_quote!(::eva_handlers))
}
