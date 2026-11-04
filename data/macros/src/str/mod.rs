use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

use crate::utils::{self, Target};

use self::args::{Args, Kind};

mod args;
mod common;
mod fixed;
mod newtype;

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let (args, input) = utils::both(
        syn::parse2::<Args>(args),
        syn::parse2::<syn::DeriveInput>(item),
    )?;

    let KindImpls {
        target,
        attrs,
        impls,
    } = match &args.kind {
        Kind::Newtype => newtype::expand(&input, &args)?,
        Kind::Fixed { error } => fixed::expand(&input, error, &args)?,
        Kind::Custom => KindImpls {
            target: Target::new(&input.ident, &input.generics),
            attrs: quote! {},
            impls: quote! {},
        },
    };
    let common = common::impls(&target, &args);
    let copy = args.copy || matches!(args.kind, Kind::Fixed { .. });
    let copy = utils::maybe(copy, || quote! { ::core::marker::Copy });

    Ok(quote! {
        #[derive(::core::clone::Clone, #copy)]
        #attrs
        #input
        #impls
        #common
    })
}

pub fn fallback(item: TokenStream) -> TokenStream {
    item
}

/// What a kind adds to the surface every kind shares.
struct KindImpls<'a> {
    /// Carries the kind's extra bounds, which every emitted impl needs.
    target: Target<'a>,
    /// Placed before the item's own attributes.
    attrs: TokenStream,
    impls: TokenStream,
}

/// Span the call at the type name instead of autodereferencing through generated `Deref`.
fn as_str(target: &Target<'_>) -> TokenStream {
    quote_spanned! {target.ident.span()=> Self::as_str}
}
