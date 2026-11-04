use std::error::Error;

use eva::{_priv::schemars, data, generic::Case};
use serde_json::json;

fn source_text(err: &dyn Error) -> Option<String> {
    err.source().map(ToString::to_string)
}

fn schema<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap()
}

mod display_struct {
    use super::*;

    #[data(display("{x}:{y}"))]
    struct Point {
        x: i32,
        y: i32,
    }

    #[data(display("{_0}/{_1}"))]
    struct Pair(u8, u8);

    #[data(display("got {}, limit {LIMIT}, sum {}", *got, self.sum()))]
    struct TooBig<const LIMIT: usize> {
        got: usize,
    }

    impl<const LIMIT: usize> TooBig<LIMIT> {
        fn sum(&self) -> usize {
            self.got + LIMIT
        }
    }

    // A field named like the macro's formatter still binds the field.
    #[data(display("{__formatter} of {n}", n = 1))]
    struct Shadow {
        __formatter: u8,
        other: u8,
    }

    #[test]
    fn binds_fields() {
        assert_eq!(Point { x: 1, y: 2 }.to_string(), "1:2");
        assert_eq!(Pair(3, 4).to_string(), "3/4");
    }

    #[test]
    fn self_const_generics_and_args() {
        assert_eq!(TooBig::<5> { got: 7 }.to_string(), "got 7, limit 5, sum 12");
    }

    #[test]
    fn formatter_is_hygienic() {
        let shadow = Shadow {
            __formatter: 3,
            other: 0,
        };
        assert_eq!(shadow.to_string(), "3 of 1");
    }
}

mod display_enum {
    use super::*;

    #[data(copy, display("{}", self.mime()))]
    enum Format {
        Json,
        Msgpack,
    }

    impl Format {
        fn mime(self) -> &'static str {
            match self {
                Self::Json => "application/json",
                Self::Msgpack => "application/msgpack",
            }
        }
    }

    #[data(copy, display(name))]
    enum Role {
        SuperAdmin,
        User,
    }

    // A string literal is a format string, not the `name` mode.
    #[data(copy, display("name"))]
    enum Literal {
        Day,
    }

    #[data(copy, error, display(doc))]
    enum Msgpack {
        /// MessagePack nested deeper than 128 levels.
        TooDeep,
        /// Invalid input.
        #[doc(hidden)]
        Invalid,
        /// Wait for it...
        Pending,
        /// Spans
        /// two lines.
        ///
        /// And a paragraph.
        Long,
    }

    #[data(display("fallback {}", self.code()))]
    enum Overridden {
        #[display("own {_0}")]
        Own(u8),
        #[display(name)]
        OwnName {
            code: u8,
        },
        #[display(doc)]
        /// From the docs.
        OwnDoc,
        Defaulted(u8),
        AlsoDefaulted,
    }

    impl Overridden {
        fn code(&self) -> u8 {
            match self {
                Self::Defaulted(code) | Self::OwnName { code } => *code,
                _ => 0,
            }
        }
    }

    #[data(display(name))]
    enum NameDefault {
        #[display("custom {value}")]
        Custom {
            value: u8,
        },
        PlainName,
    }

    #[data]
    enum PerVariant {
        #[display("got {got}, expected {}", expected + 1)]
        Mismatch { got: u8, expected: u8 },
        #[display("@{_0}")]
        Mention(String),
        #[display(doc)]
        /// Nothing here.
        Nothing,
    }

    #[test]
    fn type_level_fmt_sees_self() {
        assert_eq!(Format::Json.to_string(), "application/json");
        assert_eq!(Format::Msgpack.to_string(), "application/msgpack");
    }

    #[test]
    fn name_is_snake_case() {
        assert_eq!(Role::SuperAdmin.to_string(), "super_admin");
        assert_eq!(Role::User.to_string(), "user");
        assert_eq!(Literal::Day.to_string(), "name");
    }

    #[test]
    fn doc_text() {
        assert_eq!(
            Msgpack::TooDeep.to_string(),
            "MessagePack nested deeper than 128 levels"
        );
        assert_eq!(Msgpack::Invalid.to_string(), "invalid input");
        assert_eq!(Msgpack::Pending.to_string(), "wait for it...");
        assert_eq!(
            Msgpack::Long.to_string(),
            "spans two lines. And a paragraph"
        );
    }

    #[test]
    fn variant_overrides_default() {
        assert_eq!(Overridden::Own(1).to_string(), "own 1");
        assert_eq!(Overridden::OwnName { code: 2 }.to_string(), "own_name");
        assert_eq!(Overridden::OwnDoc.to_string(), "from the docs");
        assert_eq!(Overridden::Defaulted(3).to_string(), "fallback 3");
        assert_eq!(Overridden::AlsoDefaulted.to_string(), "fallback 0");
        assert_eq!(NameDefault::Custom { value: 4 }.to_string(), "custom 4");
        assert_eq!(NameDefault::PlainName.to_string(), "plain_name");
    }

    #[test]
    fn every_variant_annotated() {
        let mismatch = PerVariant::Mismatch {
            got: 1,
            expected: 2,
        };
        assert_eq!(mismatch.to_string(), "got 1, expected 3");
        assert_eq!(PerVariant::Mention("eva".into()).to_string(), "@eva");
        assert_eq!(PerVariant::Nothing.to_string(), "nothing here");
    }
}

// Mirrors core's `requests::tabs::list::Err`: the variant's format beats the enum-wide stub.
mod tabs {
    use super::*;

    #[data(error, display("user {user} was not found"))]
    pub struct NotFound {
        pub user: u32,
    }

    #[data(error, display("_"))]
    pub enum Err {
        #[display("{_0}")]
        NotFound(#[from] NotFound),
    }

    #[test]
    fn not_found_displays_inner_message() {
        let err = Err::from(NotFound { user: 7 });
        assert_eq!(err.to_string(), "user 7 was not found");
    }
}

mod empty {
    use super::*;

    #[data(error)]
    pub enum NoErrors {}

    #[data(error, display("_"))]
    pub enum Stub {}

    #[data(copy)]
    pub enum NotAnError {}

    impl std::fmt::Display for NotAnError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match *self {}
        }
    }

    fn is_error<E: Error>() {}

    fn is_display<T: std::fmt::Display>() {}

    #[test]
    fn empty_error_enums_implement_error() {
        is_error::<NoErrors>();
        is_error::<Stub>();
        // Without `error`, no Display is generated, so a hand-written one does not conflict.
        is_display::<NotAnError>();
    }
}

mod convert {
    use super::*;

    #[data(error, display("aux"))]
    pub struct Aux;

    #[data]
    pub enum Generic<S> {
        Aux(#[from] Aux),
        Spec(S),
    }

    #[data(error, display("wrapped {_0}"))]
    pub struct Wrapped(
        #[from]
        #[source]
        Aux,
    );

    #[data(display("{inner}"))]
    pub struct Named<T: std::fmt::Display> {
        #[from]
        inner: T,
    }

    #[data(error, not(serde, schemars, PartialEq, Clone))]
    pub enum Failure {
        #[display("io: {_0}")]
        Io(
            #[from]
            #[source]
            std::io::Error,
        ),
        #[display("boxed")]
        Boxed {
            code: u8,
            #[source]
            inner: Box<dyn Error + Send + Sync>,
        },
        #[display("plain")]
        Plain(u8),
    }

    #[test]
    fn from_variant_and_struct() {
        assert_eq!(Generic::<u8>::from(Aux), Generic::Aux(Aux));
        assert_ne!(Generic::from(Aux), Generic::Spec(1));
        assert_eq!(Wrapped::from(Aux), Wrapped(Aux));
        assert_eq!(Named::from(5).to_string(), "5");
    }

    #[test]
    fn source() {
        let io = Failure::from(std::io::Error::other("disk"));
        assert_eq!(source_text(&io).as_deref(), Some("disk"));

        let boxed = Failure::Boxed {
            code: 1,
            inner: "inner".into(),
        };
        assert_eq!(source_text(&boxed).as_deref(), Some("inner"));
        assert_eq!(source_text(&Failure::Plain(0)), None);
        assert_eq!(source_text(&Wrapped(Aux)).as_deref(), Some("aux"));
        assert_eq!(source_text(&Aux), None);
    }
}

// A lowercase const named like a macro-internal binding would turn it into a const pattern.
mod hostile_consts {
    #![allow(non_upper_case_globals, dead_code)]

    use super::*;

    const f: u8 = 0;
    const value: u8 = 0;
    const source: u8 = 0;

    #[data(error, display("inner"))]
    pub struct Inner;

    #[data(error)]
    pub enum Outer {
        #[display("outer: {_0}")]
        Inner(
            #[from]
            #[source]
            Inner,
        ),
    }

    #[test]
    fn internal_bindings_ignore_consts() {
        let err = Outer::from(Inner);
        assert_eq!(err.to_string(), "outer: inner");
        assert_eq!(source_text(&err).as_deref(), Some("inner"));
    }
}

// The macro sees variants before rustc evaluates their `#[cfg]`.
mod cfg_variants {
    use super::*;

    #[data(error, display("fallback"))]
    pub enum Gated {
        #[cfg(not(test))]
        #[display("io: {_0}")]
        Io(
            #[from]
            #[source]
            std::io::Error,
        ),
        #[cfg(not(test))]
        Defaulted,
        #[cfg(test)]
        #[display(name)]
        Kept(#[from] u8),
        Shared,
    }

    #[data(display(doc))]
    pub enum Documented {
        #[cfg(not(test))]
        /// Gone.
        Gone,
        /// Here.
        Here,
    }

    #[test]
    fn gated_variants_drop_their_arms_and_impls() {
        assert_eq!(Gated::from(1).to_string(), "kept");
        assert_eq!(Gated::Shared.to_string(), "fallback");
        assert_eq!(source_text(&Gated::Shared), None);
        assert_eq!(Gated::Kept(1).as_static_str(None), "Kept");
        assert_eq!(Documented::Here.to_string(), "here");
    }
}

mod serde_and_schema {
    use super::*;

    #[data(copy)]
    enum Role {
        SuperAdmin,
    }

    #[data]
    enum Reply {
        NotFound { x: u8 },
    }

    #[data]
    #[serde(rename_all = "camelCase")]
    struct Camel {
        foo_bar: u8,
    }

    #[data]
    #[serde(deny_unknown_fields, rename_all(serialize = "SCREAMING_SNAKE_CASE"))]
    enum Directional {
        FooBar,
    }

    #[test]
    fn snake_case_wire_format() {
        assert_eq!(
            serde_json::to_value(Role::SuperAdmin).unwrap(),
            json!("super_admin")
        );
        let reply = serde_json::to_value(Reply::NotFound { x: 1 }).unwrap();
        assert_eq!(reply, json!({ "not_found": { "x": 1 } }));
        assert_eq!(
            serde_json::from_value::<Reply>(reply).unwrap(),
            Reply::NotFound { x: 1 }
        );
    }

    #[test]
    fn user_rename_all_replaces_the_default() {
        let camel = serde_json::to_value(Camel { foo_bar: 1 }).unwrap();
        assert_eq!(camel, json!({ "fooBar": 1 }));
        assert_eq!(
            serde_json::from_value::<Camel>(camel).unwrap(),
            Camel { foo_bar: 1 }
        );
        assert_eq!(
            serde_json::to_value(Directional::FooBar).unwrap(),
            json!("FOO_BAR")
        );
    }

    #[test]
    fn schema_shape() {
        let camel = schema::<Camel>();
        assert_eq!(camel["type"], "object");
        assert_eq!(camel["properties"]["fooBar"]["type"], "integer");
        assert_eq!(schema::<Role>()["enum"], json!(["super_admin"]));
    }
}

mod derives {
    use super::*;

    #[data(not(Clone), copy)]
    struct CopyOnly;

    impl Clone for CopyOnly {
        fn clone(&self) -> Self {
            *self
        }
    }

    #[data(eq, not(PartialEq))]
    struct CustomEq(u8);

    impl PartialEq for CustomEq {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    #[data(ord, not(serde, schemars, Debug))]
    struct Ordered(u8);

    impl std::fmt::Debug for Ordered {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "#{}", self.0)
        }
    }

    #[test]
    fn not_wins_over_implied_derive() {
        assert_eq!(CopyOnly.clone(), CopyOnly);
        assert_eq!(CustomEq(1), CustomEq(2));
    }

    #[test]
    fn ord_and_not_debug() {
        assert!(Ordered(1) < Ordered(2));
        assert_eq!(format!("{:?}", Ordered(3)), "#3");
    }
}

mod static_str {
    use super::*;

    mod shadowed {
        use super::*;

        // Generated code must reach the real items through absolute paths.
        #[allow(dead_code)]
        struct Option;
        #[allow(dead_code)]
        struct Some;
        #[allow(dead_code)]
        mod core {}
        #[allow(dead_code, non_camel_case_types)]
        struct str;

        // serde's derives name `str` unqualified.
        #[data(copy, not(serde, schemars))]
        pub enum Kind {
            FooBar(u8),
            Baz { x: u8 },
        }
    }

    use shadowed::Kind;

    #[test]
    fn converts_case() {
        assert_eq!(Kind::FooBar(0).as_static_str(None), "FooBar");
        assert_eq!(Kind::FooBar(0).as_static_str(Some(Case::Snake)), "foo_bar");
        assert_eq!(Kind::FooBar(0).as_static_str(Some(Case::Kebab)), "foo-bar");
        assert_eq!(Kind::Baz { x: 0 }.as_static_str(Some(Case::Camel)), "baz");
        const PASCAL: &str = Kind::FooBar(0).as_static_str(Some(Case::Pascal));
        assert_eq!(PASCAL, "FooBar");
    }
}

#[test]
fn zst_error() {
    const LIMIT: u8 = 3;
    let err = eva::zst_error!("at most {} items", LIMIT);
    assert_eq!(err.to_string(), "at most 3 items");
}
