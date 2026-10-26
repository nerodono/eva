use eva::{handling::CallStep, service};

#[eva::data(error, display("missing"))]
pub struct Missing;

#[eva::data(error, display("invalid"))]
pub struct Invalid;

pub mod contract {
    use super::*;

    #[service(export)]
    pub trait Authors {
        #[errors(Invalid, NotFound(Missing))]
        fn get(
            id: u64,
        ) -> ok!(
            #[data(copy)]
            u64
        );

        #[flags(write)]
        fn create(name: &str) -> ok! { id: u64, name: String };
    }

    #[service]
    pub trait Health {
        fn ping() -> eva::True;

        fn status() -> ok! {
            #![data(copy)]
            #![derive(Default)]
            #![serde(rename_all = "camelCase")]

            is_ready: bool,
        };
    }
}

mod session {
    use eva::export;

    use super::contract::{Authors as Writers, Health};

    #[export]
    pub struct Session {
        authors: Writers,
        health: self::Health,
    }
}
pub use session::{Session, for_each_endpoint};

macro_rules! endpoints {
    ($($segment:ident { $accessor:ident: $Name:ident {
        trait: $Trait:path,
        client: $Client:path,
        $($(#[$flag:ident])* $method:ident = $endpoint:ident $(<$lt:lifetime>)? {
            args: $Args:ty, ok: $Ok:ty, err: $Err:ty, output: $Output:ty,
            docs: [$($doc:literal),*],
        })*
    } })*) => {
        const NAMES: &[&str] = &[$($(concat!(stringify!($segment), ".", stringify!($method)),)*)*];
        $(
            #[allow(dead_code)]
            mod $segment {
                use $Client as Client;

                fn client<S: $Trait>(state: S) -> Client<S> {
                    Client::new(state)
                }

                $(fn $method $(<$lt>)?(_: $Args) -> Option<($Ok, $Err, $Output)> {
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
    ) -> impl CallStep<contract::Authors::get::Args, Output = contract::Authors::get::Output> {
        struct Get;
        impl CallStep<contract::Authors::get::Args> for Get {
            type Output = contract::Authors::get::Output;
            async fn call(&mut self, args: contract::Authors::get::Args) -> Self::Output {
                if args.id == 0 {
                    Err(Missing.into())
                } else {
                    Ok(args.id.into())
                }
            }
        }
        Get
    }
    fn create<'a>(
        &mut self,
    ) -> impl CallStep<contract::Authors::create::Args<'a>, Output = contract::Authors::create::Output>
    {
        struct Create;
        impl<'a> CallStep<contract::Authors::create::Args<'a>> for Create {
            type Output = contract::Authors::create::Output;
            async fn call(&mut self, args: contract::Authors::create::Args<'a>) -> Self::Output {
                Ok(contract::Authors::create::Ok {
                    id: 7,
                    name: args.name.to_owned(),
                })
            }
        }
        Create
    }
}
impl contract::Health::Trait for Backend {
    fn ping(
        &mut self,
    ) -> impl CallStep<contract::Health::ping::Args, Output = contract::Health::ping::Output> {
        Ready(Ok(eva::True::new().into()))
    }

    fn status(
        &mut self,
    ) -> impl CallStep<contract::Health::status::Args, Output = contract::Health::status::Output>
    {
        Ready(Ok(contract::Health::status::Ok { is_ready: true }))
    }
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
            "health.status"
        ]
    );
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
) -> impl Future<Output = contract::Authors::create::Output> + Send {
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

    #[service(export(echo))]
    pub(crate) trait Values {
        #[errors(Absent(this![Missing]))]
        fn echo(value: this![Value<i32>]) -> ok! { value: this![Value<i32>] };
        embed! {
            #[eva::data]
            pub struct Value<T>(pub T);

            #[eva::data(error, display("missing"))]
            pub struct Missing;
        }

        fn wrap(value: Option<this![Value<i32>]>) -> this![Value<i32>];
        embed! {
            #[eva::data]
            pub struct Value<T>(pub T);
        }
    }
}

impl embedded::Values::Trait for Backend {
    fn echo(&mut self) -> impl CallStep<embedded::echo::Args, Output = embedded::echo::Output> {
        struct Echo;
        impl CallStep<embedded::echo::Args> for Echo {
            type Output = embedded::echo::Output;
            async fn call(&mut self, args: embedded::echo::Args) -> Self::Output {
                Ok(embedded::echo::Ok { value: args.value })
            }
        }
        Echo
    }

    fn wrap(
        &mut self,
    ) -> impl CallStep<embedded::Values::wrap::Args, Output = embedded::Values::wrap::Output> {
        struct Wrap;
        impl CallStep<embedded::Values::wrap::Args> for Wrap {
            type Output = embedded::Values::wrap::Output;
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
