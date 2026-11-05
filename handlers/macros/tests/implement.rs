use eva::handlers::step;

mod contracts {
    #[eva::service]
    pub trait Echo {
        fn text<'a>(value: &'a str) -> &'a str;

        fn pair<'a>(left: &'a str, right: &str) -> String;
    }

    #[eva::service]
    pub trait Counter {
        #[flags(write)]
        fn value() -> usize;
    }
}

mod api {
    use super::contracts::Echo;

    #[eva::export(flags(write))]
    pub struct Api {
        echo: self::Echo,
        counter: super::contracts::Counter,
    }
}

pub use api::{Api, for_each_endpoint};

#[eva::export(name = for_each_counter_endpoint, flags(write))]
pub struct CounterApi {
    counter: contracts::Counter,
}

mod bindings {
    pub use super::for_each_endpoint;
    use super::{contracts::Echo, step};

    pub struct Backend<'a, T, const N: usize> {
        prefix: &'a T,
        calls: usize,
    }

    impl<'a, T: AsRef<str> + Send + Sync, const N: usize> Backend<'a, T, N> {
        pub fn new(prefix: &'a T) -> Self {
            Self { prefix, calls: 0 }
        }

        fn echo_text<'m>(
            _prefix: &'a T,
            args: Echo::text::Args<'m>,
        ) -> Echo::Result<Echo::text::Ok<'m>, Echo::text::Err> {
            Ok(Echo::text::Ok(args.value))
        }

        fn echo_pair(
            prefix: &'a T,
            args: Echo::pair::Args<'_, '_>,
        ) -> Echo::Result<Echo::pair::Ok, Echo::pair::Err> {
            Ok(Echo::pair::Ok(format!(
                "{}: {}/{}",
                prefix.as_ref(),
                args.left,
                args.right,
            )))
        }

        fn next(&mut self) -> usize {
            self.calls += 1;
            N + self.calls
        }
    }

    #[eva::implement(
        self::for_each_endpoint,
        for<'a, T: AsRef<str>, const N: usize> self::Backend<'a, T, N>
        where T: Send + Sync
    )]
    #[extra(flags, lifetimes, ok)]
    macro_rules! bind {
        ($this:ident, counter, value, $Args:ty, [$($flag:ident),*], $lifetimes:tt, $Ok:ty) => {{
            let value = eva::if_flag!(write in [$($flag),*] { $this.next() } else { 0 });
            step(move |_: $Args| {
                std::future::ready(Ok(super::contracts::Counter::value::Ok(value)))
            })
        }};
        ($this:ident, $accessor:ident, $method:ident, $Args:ty, $flags:tt, [$($lt:lifetime),*], $Ok:ty) => {{
            let prefix: &'a T = $this.prefix;
            step(move |args: $Args| {
                let result: Result<$Ok, _> = Self::[<$accessor _ $method>](prefix, args);
                std::future::ready(result)
            })
        }};
    }

    pub struct Alternate;

    #[eva::implement(super::for_each_endpoint, self::Alternate)]
    #[extra(ok, flags)]
    macro_rules! bind {
        ($this:ident, echo, text, $Args:ty, $Ok:ty, $flags:tt) => {{ step(|args: $Args| std::future::ready(Ok(<$Ok>::from(args.value)))) }};
        ($this:ident, echo, pair, $Args:ty, $Ok:ty, $flags:tt) => {{
            step(|args: $Args| {
                std::future::ready(Ok(<$Ok>::from(format!(
                    "alternate: {}/{}",
                    args.left, args.right,
                ))))
            })
        }};
        ($this:ident, counter, value, $Args:ty, $Ok:ty, $flags:tt) => {{ step(|_: $Args| std::future::ready(Ok(<$Ok>::from(42)))) }};
    }
}

fn run<F: Future>(future: F) -> F::Output {
    use std::{
        pin::pin,
        task::{Context, Poll, Waker},
    };

    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test step must be immediately ready"),
    }
}

#[test]
fn generic_backend_keeps_state_and_separates_colliding_lifetimes() {
    let prefix = String::from("borrowed backend");
    let mut api = Api::new(bindings::Backend::<_, 7>::new(&prefix));

    assert_eq!(run(api.counter().value()).unwrap().0, 8);
    {
        let left = String::from("left");
        let right = String::from("right");
        assert_eq!(run(api.echo().text(&left)).unwrap().0, left);
        assert_eq!(
            run(api.echo().pair(&left, &right)).unwrap().0,
            "borrowed backend: left/right",
        );
    }
    assert_eq!(run(api.counter().value()).unwrap().0, 9);
}

#[test]
fn repeated_template_names_keep_each_implementation_body() {
    let mut alternate = Api::new(bindings::Alternate);
    assert_eq!(run(alternate.counter().value()).unwrap().0, 42);
    assert_eq!(
        run(alternate.echo().text("borrowed")).unwrap().0,
        "borrowed"
    );
    assert_eq!(
        run(alternate.echo().pair("a", "b")).unwrap().0,
        "alternate: a/b",
    );

    let prefix = String::from("original");
    let mut original = Api::new(bindings::Backend::<_, 0>::new(&prefix));
    assert_eq!(run(original.counter().value()).unwrap().0, 1);
    assert_eq!(
        run(original.echo().pair("a", "b")).unwrap().0,
        "original: a/b",
    );
}

mod matching_names {
    use super::{
        CounterApi, contracts::Counter, for_each_counter_endpoint as for_each_endpoint, run, step,
    };

    struct Backend;

    #[eva::implement(for_each_endpoint, Backend)]
    macro_rules! for_each_endpoint {
        ($this:ident, counter, value, $Args:ty) => {{
            for_each_endpoint!(@value $Args)
        }};
        (@value $Args:ty) => {{
            step(|_: $Args| std::future::ready(Ok(Counter::value::Ok(17))))
        }};
    }

    #[test]
    fn template_can_shadow_metadata_and_recurse() {
        let mut api = CounterApi::new(Backend);
        assert_eq!(run(api.counter().value()).unwrap().0, 17);
    }
}

mod empty {
    #[eva::export(name = for_each_empty_endpoint)]
    pub struct Empty {}

    struct Backend;

    #[eva::implement(self::for_each_empty_endpoint, self::Backend)]
    #[allow(unused_macros)]
    macro_rules! bind {
        ($($args:tt)*) => {
            compile_error!("an empty export must not invoke its implementation template");
        };
    }

    #[test]
    fn empty_export_does_not_invoke_the_template() {
        fn accepts_empty(_: impl EmptyFeatures) {}
        accepts_empty(Backend);
        let _: Backend = Empty::new(Backend).into_inner();
    }
}
