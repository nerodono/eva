use eva::{
    handling::{CallStep, step},
    service,
};

#[eva::data(error, display("missing"))]
pub struct Missing;

#[eva::data(error, display("invalid"))]
pub struct Invalid;

pub mod contract {
    use super::*;

    #[service(export)]
    pub trait Authors {
        #[errors(Invalid, NotFound(Missing))]
        #[ok(data(copy))]
        fn get(id: u64) -> u64;

        #[flags(write)]
        fn create(name: &str) -> Struct! { id: u64, name: String };
    }

    #[service]
    pub trait Health {
        fn ping() -> eva::True;

        fn status() -> Struct! {
            #![data(copy)]
            #![derive(Default)]
            #![serde(rename_all = "camelCase")]

            is_ready: bool,
        };
    }
}

pub mod more {
    use super::*;

    #[service(export)]
    pub trait Extras {
        #[flags(stream)]
        fn pair<'a>(left: &'a str, right: &str) -> u32;

        #[err(data(not(schemars)))]
        #[errors(Invalid)]
        fn checked();

        #[ok(data(not(serde, schemars)))]
        fn raw() -> u64;
    }
}

mod session {
    use eva::export;

    use super::contract::{Authors as Writers, Health};

    #[export(flags(write, stream))]
    pub struct Session {
        authors: Writers,
        health: self::Health,
        extras: super::more::Extras,
    }
}
pub use session::{Session, for_each_endpoint};

macro_rules! endpoints {
    ($($accessor:ident {
        trait: $Trait:path,
        client: $Client:path,
        methods: {$($method:ident {
            flags: [$($flag:ident),*],
            lifetimes: [$($lt:lifetime),*],
            args: $Args:ty, ok: $Ok:ty, err: $Err:ty, output: $Output:ty,
            docs: [$($doc:literal),*],
            $($method_extra:tt)*
        })*},
        $($extra:tt)*
    })*) => {
        const NAMES: &[&str] = &[$($(concat!(stringify!($accessor), ".", stringify!($method)),)*)*];

        #[allow(clippy::vec_init_then_push, reason = "pushes come from if_flag!")]
        fn writes() -> Vec<&'static str> {
            let mut writes = Vec::new();
            $($(eva::if_flag!(write in [$($flag),*] {
                writes.push(concat!(stringify!($accessor), ".", stringify!($method)));
            });)*)*
            writes
        }

        $(
            #[allow(dead_code)]
            mod $accessor {
                use $Client as Client;

                fn client<S: $Trait>(state: S) -> Client<S> {
                    Client::new(state)
                }

                $(fn $method<$($lt),*>(_: $Args) -> Option<($Ok, $Err, $Output)> {
                    None
                })*
            }
        )*
    };
}
for_each_endpoint!(endpoints);

struct Ready<T>(T);
impl<I: Send, T: Clone + Send + Sync> CallStep<I> for Ready<T> {
    type Output = T;
    async fn call(&mut self, _: I) -> T {
        self.0.clone()
    }
}

struct Backend;
impl contract::Authors::Trait for Backend {
    fn get(
        &mut self,
    ) -> impl CallStep<
        contract::Authors::get::Args,
        Output = contract::Authors::Result<contract::Authors::get::Ok, contract::Authors::get::Err>,
    > {
        step(|args: contract::Authors::get::Args| async move {
            if args.id == 0 {
                Err(Missing.into())
            } else {
                Ok(args.id.into())
            }
        })
    }
    fn create<'a>(
        &mut self,
    ) -> impl CallStep<
        contract::Authors::create::Args<'a>,
        Output = contract::Authors::Result<
            contract::Authors::create::Ok,
            contract::Authors::create::Err,
        >,
    > {
        step(|args: contract::Authors::create::Args<'a>| async move {
            Ok(contract::Authors::create::Ok {
                id: 7,
                name: args.name.to_owned(),
            })
        })
    }
}
impl contract::Health::Trait for Backend {
    fn ping(
        &mut self,
    ) -> impl CallStep<
        contract::Health::ping::Args,
        Output = contract::Health::Result<contract::Health::ping::Ok, contract::Health::ping::Err>,
    > {
        Ready(Ok(eva::True::new().into()))
    }

    fn status(
        &mut self,
    ) -> impl CallStep<
        contract::Health::status::Args,
        Output = contract::Health::Result<
            contract::Health::status::Ok,
            contract::Health::status::Err,
        >,
    > {
        Ready(Ok(contract::Health::status::Ok { is_ready: true }))
    }
}

impl more::Extras::Trait for Backend {
    fn pair<'a, 'b>(
        &mut self,
    ) -> impl CallStep<
        more::pair::Args<'a, 'b>,
        Output = more::Extras::Result<more::pair::Ok, more::pair::Err>,
    > {
        Ready(Ok(2.into()))
    }

    fn checked(
        &mut self,
    ) -> impl CallStep<
        more::checked::Args,
        Output = more::Extras::Result<more::checked::Ok, more::checked::Err>,
    > {
        Ready(Err(Invalid.into()))
    }

    fn raw(
        &mut self,
    ) -> impl CallStep<more::raw::Args, Output = more::Extras::Result<more::raw::Ok, more::raw::Err>>
    {
        Ready(Ok(5.into()))
    }
}

#[test]
fn custom_data_keeps_errors_and_skips_serde_transparency() {
    fn error<E: std::error::Error>(_: &E) {}
    let mut session = Session::new(Backend);
    assert_eq!(run(session.extras().pair("a", "b")).unwrap().0, 2);
    let err = run(session.extras().checked()).unwrap_err();
    error(&err);
    assert_eq!(err.to_string(), "invalid");
    assert_eq!(run(session.extras().raw()).unwrap().0, 5);
}

fn run<F: Future>(future: F) -> F::Output {
    use std::{
        pin::pin,
        task::{Context, Poll, Waker},
    };
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test endpoint must be immediately ready"),
    }
}

#[test]
fn clients_accept_owned_borrowed_and_boxed_implementations() {
    assert_eq!(
        NAMES,
        [
            "authors.get",
            "authors.create",
            "health.ping",
            "health.status",
            "extras.pair",
            "extras.checked",
            "extras.raw",
        ]
    );
    assert_eq!(writes(), ["authors.create"]);
    let mut session = Session::new(Backend);
    assert_eq!(run(session.authors().get(4)).unwrap().0, 4);
    assert!(matches!(
        run(session.authors().get(0)),
        Err(contract::Authors::get::Err::NotFound(_))
    ));
    let input = String::from("Ada");
    assert_eq!(run(session.authors().create(&input)).unwrap().name, input);
    assert_eq!(run(session.health().ping()).unwrap().0, eva::True::new());
    let mut owned = contract::Authors::Client::new(Backend);
    assert_eq!(run(owned.get(2)).unwrap().0, 2);
    let mut boxed = contract::Authors::Client::new(Box::new(Backend));
    assert_eq!(run(boxed.get(3)).unwrap().0, 3);
}

#[allow(
    clippy::manual_async_fn,
    reason = "The signature verifies that generic callers return a Send future."
)]
fn borrowed_generic_call<S: contract::Authors::Trait>(
    state: &mut S,
    input: &str,
) -> impl Future<
    Output = contract::Authors::Result<
        contract::Authors::create::Ok,
        contract::Authors::create::Err,
    >,
> + Send {
    async move { contract::Authors::Client::new(state).create(input).await }
}
#[test]
fn borrowed_clients_remain_send_in_generic_handlers() {
    assert_eq!(
        run(borrowed_generic_call(&mut Backend, "Ada")).unwrap().id,
        7
    );
}

#[test]
fn generated_types_preserve_named_arguments_and_transparent_successes() {
    use contract::{create, get};
    assert_eq!(
        serde_json::to_value(get::Args { id: 4 }).unwrap(),
        serde_json::json!({ "id": 4 })
    );
    assert_eq!(serde_json::to_value(get::Ok::from(4)).unwrap(), 4);
    assert_eq!(serde_json::from_str::<get::Ok>("4").unwrap().0, 4);
    let id = get::Ok::from(4);
    let copy = id;
    assert_eq!(id, copy);
    assert_eq!(
        serde_json::to_value(create::Ok {
            id: 7,
            name: "Ada".into()
        })
        .unwrap(),
        serde_json::json!({ "id": 7, "name": "Ada" })
    );
}

mod embedded {
    use eva::service;

    macro_rules! ty {
        ($ty:ty) => {
            $ty
        };
    }

    #[service(export(echo))]
    pub(crate) trait Values {
        #[errors(Absent(this::Missing))]
        fn echo(value: this::Value<i32>) -> Struct! { value: this::Value<i32> };
        embed! {
            #[eva::data]
            pub struct Value<T>(pub T);

            #[eva::data(error, display("missing"))]
            pub struct Missing;
        }

        fn wrap(value: Option<ty!(this::nested::Value)>) -> ty!(this::nested::Value);
        embed! {
            #[eva::data]
            pub struct Value<T>(pub T);

            pub mod nested {
                use super::this;

                pub type Value = this::Value<i32>;
            }
        }
    }
}

impl embedded::Values::Trait for Backend {
    fn echo(
        &mut self,
    ) -> impl CallStep<
        embedded::echo::Args,
        Output = embedded::Values::Result<embedded::echo::Ok, embedded::echo::Err>,
    > {
        struct Echo;
        impl CallStep<embedded::echo::Args> for Echo {
            type Output = embedded::Values::Result<embedded::echo::Ok, embedded::echo::Err>;
            async fn call(&mut self, args: embedded::echo::Args) -> Self::Output {
                Ok(embedded::echo::Ok { value: args.value })
            }
        }
        Echo
    }

    fn wrap(
        &mut self,
    ) -> impl CallStep<
        embedded::Values::wrap::Args,
        Output = embedded::Values::Result<embedded::Values::wrap::Ok, embedded::Values::wrap::Err>,
    > {
        struct Wrap;
        impl CallStep<embedded::Values::wrap::Args> for Wrap {
            type Output =
                embedded::Values::Result<embedded::Values::wrap::Ok, embedded::Values::wrap::Err>;
            async fn call(&mut self, args: embedded::Values::wrap::Args) -> Self::Output {
                Ok(args
                    .value
                    .unwrap_or(embedded::Values::wrap::Value(0))
                    .into())
            }
        }
        Wrap
    }
}

#[test]
fn embedded_generic_types_resolve_in_arguments_responses_and_clients() {
    use embedded::{
        Values::{Client, wrap},
        echo,
    };
    let mut client = Client::new(Backend);
    assert_eq!(run(client.echo(echo::Value(42))).unwrap().value.0, 42);
    assert_eq!(run(client.wrap(Some(wrap::Value(7)))).unwrap().0.0, 7);
    assert!(matches!(
        embedded::echo::Err::from(embedded::echo::Missing),
        embedded::echo::Err::Absent(_)
    ));
}

#[test]
fn inner_success_attributes_customize_the_generated_structure() {
    use contract::Health::status;
    fn copy<T: Copy>(value: T) -> (T, T) {
        (value, value)
    }
    let value = run(Session::new(Backend).health().status()).unwrap();
    assert_eq!(copy(value), (value, value));
    assert!(!status::Ok::default().is_ready);
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::json!({ "isReady": true })
    );
}

#[test]
fn error_aliases_preserve_conversions_display_and_serialization() {
    use contract::get;
    assert!(matches!(get::Err::from(Invalid), get::Err::Invalid(_)));
    let error = get::Err::from(Missing);
    assert_eq!(error.to_string(), "missing");
    let json = serde_json::to_string(&error).unwrap();
    assert!(matches!(
        serde_json::from_str::<get::Err>(&json).unwrap(),
        get::Err::NotFound(_)
    ));
}

mod inline_responses {
    use super::{CallStep, run, step};
    use eva::handling::Endpoint;

    #[eva::service(export)]
    pub trait Values {
        fn inspect<'a>(
            value: &'a str,
        ) -> Enum! {
            #![data(copy)]
            #![derive(Default)]
            #![serde(rename_all = "snake_case")]

            /// A response with a named field.
            Record {
                #[serde(rename = "text")]
                value: &'a str,
            },
            Tuple(#[serde(borrow)] &'a str),
            #[default]
            #[serde(rename = "empty")]
            Simple,
            #[cfg(any())]
            Disabled(Undefined),
        };

        fn record<'a>(
            value: &'a str,
        ) -> Struct! {
            #![data(copy)]

            #[serde(rename = "text", default = "this::default_value")]
            value: &'a str,
        };
        embed! {
            fn default_value() -> &'static str {
                "default"
            }
        }

        fn type_of_num(
            num: i8,
        ) -> Enum! {
            #![data(copy)]
            #![repr(i8)]

            Even = 2,
            Odd = 3,
        };
    }

    #[eva::endpoint(Values::inspect)]
    async fn Inspect<'a>(_state: (), args: this::Args<'a>) -> this::Output<'a> {
        Ok(match args.value {
            "" => this::Ok::Simple,
            "tuple" => this::Ok::Tuple(args.value),
            value => this::Ok::Record { value },
        })
    }

    struct Backend;

    impl Values::Trait for Backend {
        fn inspect<'a>(
            &mut self,
        ) -> impl CallStep<inspect::Args<'a>, Output = Values::Result<inspect::Ok<'a>, inspect::Err>>
        {
            step(|args: inspect::Args<'a>| Inspect.call((), args))
        }

        fn record<'a>(
            &mut self,
        ) -> impl CallStep<record::Args<'a>, Output = Values::Result<record::Ok<'a>, record::Err>>
        {
            step(async |args: record::Args<'a>| Ok(record::Ok { value: args.value }))
        }

        fn type_of_num(
            &mut self,
        ) -> impl CallStep<type_of_num::Args, Output = Values::Result<type_of_num::Ok, type_of_num::Err>>
        {
            step(async |args: type_of_num::Args| {
                Ok(if args.num % 2 == 0 {
                    type_of_num::Ok::Even
                } else {
                    type_of_num::Ok::Odd
                })
            })
        }
    }

    #[test]
    fn inline_responses_preserve_borrows_attributes_and_variant_shapes() {
        let text = String::from("borrowed response");
        let mut client = Values::Client::new(Backend);
        let record = run(client.inspect(&text)).unwrap();
        assert_eq!(record, inspect::Ok::Record { value: &text });
        let copy = record;
        assert_eq!(record, copy);
        let tuple = run(client.inspect("tuple")).unwrap();
        assert_eq!(tuple, inspect::Ok::Tuple("tuple"));
        assert_eq!(run(client.inspect("")).unwrap(), inspect::Ok::default());
        for (value, expected) in [
            (record, serde_json::json!({ "record": { "text": text } })),
            (tuple, serde_json::json!({ "tuple": "tuple" })),
            (inspect::Ok::Simple, serde_json::json!("empty")),
        ] {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::to_value(value).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<inspect::Ok<'_>>(&json).unwrap(),
                value
            );
        }

        let record = run(client.record(&text)).unwrap();
        assert_eq!(record.value, text);
        assert_eq!(
            serde_json::to_value(record).unwrap(),
            serde_json::json!({ "text": text })
        );
        assert_eq!(
            serde_json::from_str::<record::Ok<'_>>("{}").unwrap().value,
            "default"
        );
        assert_eq!(run(client.type_of_num(-2)).unwrap(), type_of_num::Ok::Even);
        assert_eq!(run(client.type_of_num(3)).unwrap(), type_of_num::Ok::Odd);
        assert_eq!(type_of_num::Ok::Even as i8, 2);
        assert_eq!(type_of_num::Ok::Odd as i8, 3);
    }
}

pub(crate) use custom_result::for_each_custom_endpoint;

mod custom_result {
    use super::{CallStep, Invalid, Missing, run, step};
    use eva::handling::Endpoint;

    #[derive(Debug)]
    pub struct Response<T, E>(pub std::result::Result<T, E>);

    // The configured constructor may itself be imported as `Result`.
    use Response as Result;

    #[eva::service(result = Result, error = Missing, export)]
    pub trait Values {
        fn echo<'a>(value: &'a str) -> &'a str;

        #[error(Invalid)]
        fn checked(value: u32) -> u32;
    }

    #[eva::export(name = for_each_custom_endpoint)]
    pub(crate) struct Session {
        values: Values,
    }

    macro_rules! compose {
        ($accessor:ident {
            trait: $Trait:path,
            client: $Client:path,
            methods: {$($method:ident {
                flags: [$($flag:ident),*],
                lifetimes: [$($lt:lifetime),*],
                args: $Args:ty, ok: $Ok:ty, err: $Err:ty, output: $Output:ty,
                docs: [$($doc:literal),*],
            })*},
        }) => {
            mod composed {
                $(pub fn $method<$($lt),*>(value: $Ok) -> $Output {
                    super::Response(Ok(value))
                })*
            }
        };
    }
    for_each_custom_endpoint!(compose);

    #[eva::endpoint(self::Values::echo)]
    async fn Echo<'a>(_state: (), args: this::Args<'a>) -> this::Output<'a> {
        let _: Option<this::Err> = None::<Missing>;
        Result(Ok(this::Ok(args.value)))
    }

    use Values::checked as renamed_method;

    #[eva::endpoint(renamed_method)]
    async fn Checked(_state: (), args: this::Args) -> this::Output {
        let error: this::Err = Invalid;
        if args.value == 0 {
            Result(Err(error))
        } else {
            Result(Ok(this::Ok(args.value)))
        }
    }

    struct Backend;

    impl Values::Trait for Backend {
        fn echo<'a>(
            &mut self,
        ) -> impl CallStep<echo::Args<'a>, Output = Values::Result<echo::Ok<'a>, echo::Err>>
        {
            step(|args: echo::Args<'a>| Echo.call((), args))
        }

        fn checked(
            &mut self,
        ) -> impl CallStep<checked::Args, Output = Values::Result<checked::Ok, checked::Err>>
        {
            step(|args: checked::Args| Checked.call((), args))
        }
    }

    #[test]
    fn result_container_and_method_errors_agree_across_all_consumers() {
        let text = String::from("borrowed result");
        let mut session = Session::new(Backend);
        assert_eq!(run(session.values().echo(&text)).0.unwrap().0, text);
        assert_eq!(run(session.values().checked(42)).0.unwrap().0, 42);
        assert_eq!(run(session.values().checked(0)).0.unwrap_err(), Invalid);
        assert_eq!(composed::echo(echo::Ok(&text)).0.unwrap().0, text);
        assert_eq!(composed::checked(checked::Ok(7)).0.unwrap().0, 7);
    }
}
