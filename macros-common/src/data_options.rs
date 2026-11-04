use proc_macro2::TokenStream;

use syn::parse::{Parse, ParseStream};

use crate::{
    opts::{self, CommonTraits, Key, Opts, Table},
    utils,
};

use syn::Token;

const OPTIONS: Table = Table {
    keys: &[
        Key::flag("error"),
        Key::flag("copy"),
        Key::flag("eq"),
        Key::flag("ord"),
        Key::list("display"),
        opts::CRATE,
    ],
    not: &[opts::COMMON_NOT, &["Clone", "PartialEq", "as_static_str"]],
};

pub struct Args {
    pub eva: Option<syn::Path>,
    pub error: bool,
    pub derives: Derives,
    /// The `as_static_str` key of `not(...)`, kept for its span.
    not_as_static_str: Option<syn::Ident>,
    pub display: Option<DisplayArg>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let opts = Opts::parse(input, &OPTIONS)?;
        let args = Self {
            eva: opts.krate(),
            error: opts.flag("error").is_some(),
            derives: Derives::resolve(&opts),
            not_as_static_str: opts.not("as_static_str").cloned(),
            display: opts.list("display", DisplayArg::parse)?,
        };
        args.check_crate()?;
        Ok(args)
    }
}

impl Args {
    pub fn serde(&self) -> bool {
        self.derives.common.serialize || self.derives.common.deserialize
    }

    pub fn as_static_str(&self) -> bool {
        self.not_as_static_str.is_none()
    }

    /// Rejects options that don't apply to the item kind.
    pub fn check(&self, is_enum: bool) -> syn::Result<()> {
        match (is_enum, &self.not_as_static_str) {
            (false, Some(key)) => Err(syn::Error::new(
                key.span(),
                "`not(as_static_str)` only applies to enums",
            )),
            _ => Ok(()),
        }
    }

    fn check_crate(&self) -> syn::Result<()> {
        let d = &self.derives.common;
        if let Some(eva) = &self.eva
            && utils::is_dollar_crate(eva)
            && (d.serialize || d.deserialize || d.json_schema)
        {
            return Err(syn::Error::new_spanned(
                eva,
                "a `$crate` path cannot be passed to the serde and schemars derives; \
                 opt out of them with `not(...)`",
            ));
        }
        Ok(())
    }
}

pub struct Derives {
    pub common: CommonTraits,
    pub clone: bool,
    pub copy: bool,
    pub partial_eq: bool,
    pub eq: bool,
    pub partial_ord: bool,
    pub ord: bool,
}

impl Derives {
    /// `copy` implies Clone and `eq`/`ord` imply PartialEq, which are on by default, so
    /// `not(...)` always wins over them.
    fn resolve(opts: &Opts) -> Self {
        let ord = opts.flag("ord").is_some();
        Self {
            common: opts.common(),
            clone: opts.not("Clone").is_none(),
            copy: opts.flag("copy").is_some(),
            partial_eq: opts.not("PartialEq").is_none(),
            eq: ord || opts.flag("eq").is_some(),
            partial_ord: ord,
            ord,
        }
    }
}

/// `"fmt", args...`, `name` or `doc`.
pub enum DisplayArg {
    Fmt {
        fmt: syn::LitStr,
        args: Vec<syn::Expr>,
    },
    /// The snake_case variant name.
    Name(syn::Ident),
    /// The variant's doc comment.
    Doc(syn::Ident),
}

impl Parse for DisplayArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        const EXPECTED: &str = "expected a format string, `name` or `doc`";
        if input.peek(syn::LitStr) {
            return parse_fmt(input);
        }
        let mode = input
            .parse::<syn::Ident>()
            .map_err(|e| syn::Error::new(e.span(), EXPECTED))?;
        let display = match mode.to_string().as_str() {
            "name" => Self::Name(mode.clone()),
            "doc" => Self::Doc(mode.clone()),
            _ => return Err(syn::Error::new(mode.span(), EXPECTED)),
        };
        if !input.is_empty() {
            let rest: TokenStream = input.parse()?;
            let msg = format!("`{mode}` takes no arguments");
            return Err(syn::Error::new_spanned(rest, msg));
        }
        Ok(display)
    }
}

fn parse_fmt(input: ParseStream) -> syn::Result<DisplayArg> {
    let fmt = input.parse()?;
    let mut args = Vec::new();
    while !input.is_empty() {
        input.parse::<Token![,]>()?;
        if input.is_empty() {
            break;
        }
        args.push(input.parse()?);
    }
    Ok(DisplayArg::Fmt { fmt, args })
}
