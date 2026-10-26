mod inherent;
mod newtype;
mod ranges;
mod repr;
mod traits;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Token,
    parse::{ParseStream, Parser as _},
};

use self::{ranges::Ranges, repr::Repr};
use crate::{
    opts::{self, CommonTraits, Opts, Table},
    utils::{self, Errors},
};

const OPTIONS: Table = Table {
    keys: &[opts::CRATE],
    not: &[opts::COMMON_NOT, &["Display"]],
};

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if newtype::applies(&args) {
        return newtype::expand(args, item);
    }

    // The newtype hint only when the args are not the problem, e.g. `newtype` after an option.
    let args = Args::parse.parse2(args);
    let (item, args) = utils::both(parse_item(item, args.is_ok()), args)?;
    let int = Int { item, args };

    let definition = int.definition();
    let inherent = int.inherent();
    let traits = int.traits();
    Ok(quote! {
        #definition
        #inherent
        #traits
    })
}

/// The item without generics, which would add unused-parameter errors to an enum.
pub fn fallback(item: TokenStream) -> TokenStream {
    match syn::parse2::<syn::ItemEnum>(item.clone()) {
        Ok(mut item) => {
            item.generics = syn::Generics::default();
            quote!(#item)
        }
        Err(_) => item,
    }
}

/// The annotated enum, which must be empty: the macro generates the variants.
fn parse_item(item: TokenStream, hint_newtype: bool) -> syn::Result<syn::ItemEnum> {
    let input = syn::parse2::<syn::DeriveInput>(item)?;
    let data = match input.data {
        syn::Data::Enum(data) => data,
        syn::Data::Struct(data) => {
            let msg = if hint_newtype {
                "`#[int(<repr>, <ranges>)]` requires an enum; a struct takes `#[int(newtype)]`"
            } else {
                "`#[int(<repr>, <ranges>)]` requires an enum"
            };
            return Err(syn::Error::new(data.struct_token.span, msg));
        }
        syn::Data::Union(data) => {
            let msg = "`#[int(<repr>, <ranges>)]` requires an enum";
            return Err(syn::Error::new(data.union_token.span, msg));
        }
    };
    let item = syn::ItemEnum {
        attrs: input.attrs,
        vis: input.vis,
        enum_token: data.enum_token,
        ident: input.ident,
        generics: input.generics,
        brace_token: data.brace_token,
        variants: data.variants,
    };

    let mut errors = Errors::default();
    if !item.variants.is_empty() {
        let msg = "#[int] generates the variants; leave the enum body empty";
        errors.push(syn::Error::new_spanned(&item.variants, msg));
    }
    if !item.generics.params.is_empty() {
        let msg = "#[int] enums cannot be generic";
        errors.push(syn::Error::new_spanned(&item.generics, msg));
    }
    if let Some(where_clause) = &item.generics.where_clause {
        let msg = "#[int] enums cannot have a where clause";
        errors.push(syn::Error::new_spanned(where_clause, msg));
    }
    errors.finish()?;
    Ok(item)
}

struct Args {
    repr: Repr,
    ranges: Ranges,
    eva: syn::Path,
    traits: CommonTraits,
    display: bool,
}

impl Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let repr_span = input.span();
        let repr: Repr = input.parse()?;
        if input.is_empty() {
            let msg = "expected `, <ranges>` after the repr, as in `#[int(u8, 0..=9)]`";
            return Err(syn::Error::new(repr_span, msg));
        }
        let comma: Token![,] = input.parse()?;
        if input.is_empty() {
            return Err(syn::Error::new_spanned(comma, "expected ranges after `,`"));
        }
        let written = ranges::parse(input)?;
        let opts = Opts::parse_after_positional(input, &OPTIONS);
        let (ranges, opts) = utils::both(Ranges::resolve(&written, repr), opts)?;

        Ok(Self {
            repr,
            ranges,
            eva: opts.krate(),
            traits: opts.common(),
            display: opts.not("Display").is_none(),
        })
    }
}

struct Int {
    item: syn::ItemEnum,
    args: Args,
}

impl Int {
    fn definition(&self) -> TokenStream {
        let syn::ItemEnum {
            attrs, vis, ident, ..
        } = &self.item;
        let repr = self.args.repr.ident();
        let variants = self.args.ranges.values().map(|value| {
            let variant = variant(value);
            let value = ranges::literal(value);
            quote!(#variant = #value)
        });

        // A derived `PartialEq` lets the variants be used in patterns.
        quote! {
            #[repr(#repr)]
            #[derive(::core::cmp::PartialEq, ::core::cmp::Eq)]
            #(#attrs)*
            #vis enum #ident {
                #(#variants),*
            }
        }
    }
}

/// `POS<n>` for `n`, `NEG<n>` for `-n`.
fn variant(value: i128) -> syn::Ident {
    let sign = if value < 0 { "NEG" } else { "POS" };
    format_ident!("{sign}{}", value.unsigned_abs())
}
