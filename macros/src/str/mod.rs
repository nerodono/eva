use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

use crate::utils;

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
            target: Target::new(&input),
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

/// Impl generics, the type with its generic arguments, and the where-clause.
type Split<'a> = (
    syn::ImplGenerics<'a>,
    TokenStream,
    Option<&'a syn::WhereClause>,
);

/// The annotated type as the emitted impls name it.
struct Target<'a> {
    ident: &'a syn::Ident,
    generics: syn::Generics,
}

impl<'a> Target<'a> {
    fn new(input: &'a syn::DeriveInput) -> Self {
        Self {
            ident: &input.ident,
            generics: input.generics.clone(),
        }
    }

    fn split(&self) -> Split<'_> {
        let ident = self.ident;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        (impl_generics, quote! { #ident #ty_generics }, where_clause)
    }

    /// `Self::as_str` as a path call spanned at the type name, so a missing `as_str` is an error
    /// there instead of autoderef through the emitted `Deref<Target = str>`.
    fn as_str(&self) -> TokenStream {
        quote_spanned! {self.ident.span()=> Self::as_str}
    }

    /// The generics with `lifetime` prepended, for impls that need a lifetime of their own.
    fn with_lifetime(&self, lifetime: &syn::Lifetime) -> syn::Generics {
        let mut generics = self.generics.clone();
        let param = syn::LifetimeParam::new(lifetime.clone());
        generics
            .params
            .insert(0, syn::GenericParam::Lifetime(param));
        generics
    }
}

fn requires_struct<'a>(input: &'a syn::DeriveInput, kind: &str) -> syn::Result<&'a syn::Fields> {
    let keyword = match &input.data {
        syn::Data::Struct(data) => return Ok(&data.fields),
        syn::Data::Enum(data) => data.enum_token.span,
        syn::Data::Union(data) => data.union_token.span,
    };
    Err(syn::Error::new(
        keyword,
        format!("`{kind}` requires a struct"),
    ))
}

/// `{ a: v1, b: v2 }`, `(v1, v2)` or nothing, matching the shape of `fields`.
fn construct(fields: &syn::Fields, values: impl Iterator<Item = TokenStream>) -> TokenStream {
    let values = fields
        .iter()
        .zip(values)
        .map(|(field, value)| match &field.ident {
            Some(name) => quote! { #name: #value },
            None => value,
        });
    match fields {
        syn::Fields::Named(_) => quote! { { #(#values),* } },
        syn::Fields::Unnamed(_) => quote! { ( #(#values),* ) },
        syn::Fields::Unit => quote! {},
    }
}
