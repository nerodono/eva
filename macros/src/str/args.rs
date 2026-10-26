use syn::{
    ext::IdentExt as _,
    parse::{Parse, ParseStream},
    token,
};

use crate::{
    opts::{self, Key, Opts, Table},
    utils::Errors,
};

const OPTIONS: Table = Table {
    keys: &[Key::flag("copy"), opts::CRATE],
    not: &[opts::COMMON_NOT, &["Display", "pat"]],
};

const FIXED_OPTIONS: Table = Table {
    keys: &[Key::value::<syn::Type>("error")],
    not: &[],
};

const KIND_EXPECTED: &str =
    "the first argument must be the kind: `newtype`, `custom` or `fixed(error = Type)`";

const FIXED_EXPECTED: &str = "`fixed` requires an error type: `fixed(error = Type)`";

pub struct Args {
    pub kind: Kind,
    pub copy: bool,
    pub eva: syn::Path,
    pub traits: Traits,
}

/// The optional traits that are not opted out of.
pub struct Traits {
    pub serialize: bool,
    pub deserialize: bool,
    pub json_schema: bool,
    pub debug: bool,
    pub display: bool,
    pub pattern: bool,
}

pub enum Kind {
    /// Delegates to its only field.
    Newtype,
    /// `#[repr(C)]` concatenation of `FixedUtf8` fields.
    Fixed { error: Box<syn::Type> },
    /// The user provides `as_str` and `FromStr`.
    Custom,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let kind = input.parse::<Kind>()?;
        let opts = Opts::parse_after_positional(input, &OPTIONS)?;
        check_kind_options(&kind, &opts)?;

        let common = opts.common();
        Ok(Self {
            kind,
            copy: opts.flag("copy").is_some(),
            eva: opts.krate(),
            traits: Traits {
                serialize: common.serialize,
                deserialize: common.deserialize,
                json_schema: common.json_schema,
                debug: common.debug,
                display: opts.not("Display").is_none(),
                pattern: opts.not("pat").is_none(),
            },
        })
    }
}

/// Rejects `not(...)` keys that have no effect for the kind.
fn check_kind_options(kind: &Kind, opts: &Opts) -> syn::Result<()> {
    let mut errors = Errors::default();
    if let Some(key) = opts.not("pat")
        && !matches!(kind, Kind::Newtype)
    {
        let msg = "`not(pat)` only applies to `newtype`: `fixed` always implements `HasPattern` and `custom` never does";
        errors.push(syn::Error::new(key.span(), msg));
    }
    if matches!(kind, Kind::Custom) {
        for key in ["schemars", "JsonSchema"]
            .map(|name| opts.not(name))
            .into_iter()
            .flatten()
        {
            let msg = format!("`not({key})` has no effect: `custom` never implements `JsonSchema`");
            errors.push(syn::Error::new(key.span(), msg));
        }
    }
    errors.finish()
}

impl Parse for Kind {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if !input.peek(syn::Ident::peek_any) {
            return Err(input.error(KIND_EXPECTED));
        }
        let kind = input.call(syn::Ident::parse_any)?;
        if kind == "newtype" {
            Ok(Self::Newtype)
        } else if kind == "custom" {
            Ok(Self::Custom)
        } else if kind == "fixed" {
            parse_fixed(&kind, input)
        } else {
            Err(syn::Error::new(kind.span(), KIND_EXPECTED))
        }
    }
}

fn parse_fixed(kind: &syn::Ident, input: ParseStream) -> syn::Result<Kind> {
    if !input.peek(token::Paren) {
        return Err(syn::Error::new(kind.span(), FIXED_EXPECTED));
    }
    let content;
    let paren = syn::parenthesized!(content in input);
    let opts = Opts::parse(&content, &FIXED_OPTIONS)?;
    let error = opts
        .value::<syn::Type>("error")
        .ok_or_else(|| syn::Error::new(paren.span.join(), FIXED_EXPECTED))?;
    Ok(Kind::Fixed {
        error: Box::new(error),
    })
}
