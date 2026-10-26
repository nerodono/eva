use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::parse::{Parse, ParseStream};

use crate::{
    opts::{self, CommonTraits, Key, Opts, Table},
    utils::{self, maybe},
};

use super::{Item, Kind, display::DisplayArg};

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
    pub eva: syn::Path,
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
    pub fn as_static_str(&self) -> bool {
        self.not_as_static_str.is_none()
    }

    /// Rejects options that don't apply to the item kind.
    pub fn check(&self, item: &Item) -> syn::Result<()> {
        match (&item.kind, &self.not_as_static_str) {
            (Kind::Struct(_), Some(key)) => Err(syn::Error::new(
                key.span(),
                "`not(as_static_str)` only applies to enums",
            )),
            _ => Ok(()),
        }
    }

    fn check_crate(&self) -> syn::Result<()> {
        let d = &self.derives;
        if utils::is_dollar_crate(&self.eva) && (d.serialize || d.deserialize || d.json_schema) {
            return Err(syn::Error::new_spanned(
                &self.eva,
                "a `$crate` path cannot be passed to the serde and schemars derives; \
                 opt out of them with `not(...)`",
            ));
        }
        Ok(())
    }
}

pub struct Derives {
    serialize: bool,
    deserialize: bool,
    json_schema: bool,
    debug: bool,
    clone: bool,
    copy: bool,
    partial_eq: bool,
    eq: bool,
    partial_ord: bool,
    ord: bool,
}

impl Derives {
    /// `copy` implies Clone and `eq`/`ord` imply PartialEq, which are on by default, so
    /// `not(...)` always wins over them.
    fn resolve(opts: &Opts) -> Self {
        let ord = opts.flag("ord").is_some();
        let CommonTraits {
            serialize,
            deserialize,
            json_schema,
            debug,
        } = opts.common();
        Self {
            serialize,
            deserialize,
            json_schema,
            debug,
            clone: opts.not("Clone").is_none(),
            copy: opts.flag("copy").is_some(),
            partial_eq: opts.not("PartialEq").is_none(),
            eq: ord || opts.flag("eq").is_some(),
            partial_ord: ord,
            ord,
        }
    }

    /// The derive attributes, plus serde's and schemars' container attributes.
    pub fn attrs(&self, eva: &syn::Path, item_attrs: &[syn::Attribute]) -> TokenStream {
        let std_derives = [
            (self.debug, quote! { ::core::fmt::Debug }),
            (self.clone, quote! { ::core::clone::Clone }),
            (self.copy, quote! { ::core::marker::Copy }),
            (self.partial_eq, quote! { ::core::cmp::PartialEq }),
            (self.eq, quote! { ::core::cmp::Eq }),
            (self.partial_ord, quote! { ::core::cmp::PartialOrd }),
            (self.ord, quote! { ::core::cmp::Ord }),
        ];
        let std_derives = std_derives
            .into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, path)| quote! { #[derive(#path)] });

        let serde = quote! { #eva::_priv::serde };
        let serde_attrs = maybe(self.serialize || self.deserialize, || {
            let krate = serde.to_string();
            let ser = maybe(self.serialize, || quote! { #[derive(#serde::Serialize)] });
            let de = maybe(
                self.deserialize,
                || quote! { #[derive(#serde::Deserialize)] },
            );
            let rename = maybe(!has_rename_all(item_attrs), || {
                quote! { #[serde(rename_all = "snake_case")] }
            });
            quote! {
                #ser
                #de
                #[serde(crate = #krate)]
                #rename
            }
        });

        let json_schema = maybe(self.json_schema, || {
            let schemars = quote! { #eva::_priv::schemars };
            let krate = schemars.to_string();
            quote! {
                #[derive(#schemars::JsonSchema)]
                #[schemars(crate = #krate)]
            }
        });

        quote! {
            #(#std_derives)*
            #serde_attrs
            #json_schema
        }
    }
}

/// Whether a container `#[serde(...)]` already sets `rename_all`, in any form.
fn has_rename_all(attrs: &[syn::Attribute]) -> bool {
    let sets_rename_all = |list: &syn::MetaList| {
        let mut entry_start = true;
        list.tokens.clone().into_iter().any(|tt| {
            let found = entry_start && matches!(&tt, TokenTree::Ident(i) if i == "rename_all");
            entry_start = matches!(&tt, TokenTree::Punct(p) if p.as_char() == ',');
            found
        })
    };
    attrs.iter().any(|attr| match &attr.meta {
        syn::Meta::List(list) => list.path.is_ident("serde") && sets_rename_all(list),
        _ => false,
    })
}
