//! `From` for `#[from]` fields and `Error` with `source()` for `#[source]` fields.

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned as _;

use crate::utils::local;

use super::{Item, Kind, cases::Case};

pub fn from_impls(item: &Item, cases: &[Case]) -> TokenStream {
    let (ig, tyg, where_clause) = item.generics.split_for_impl();
    let ident = &item.ident;
    let value = local("value");
    cases
        .iter()
        .filter_map(|case| {
            let ty = &case.from?.ty;
            let construct = case.construct(&value);
            let cfg = &case.cfg;
            Some(quote! {
                #cfg
                #[automatically_derived]
                impl #ig ::core::convert::From<#ty> for #ident #tyg #where_clause {
                    #[inline]
                    fn from(#value: #ty) -> Self {
                        #construct
                    }
                }
            })
        })
        .collect()
}

pub fn error_impl(item: &Item, cases: &[Case], eva: &syn::Path) -> TokenStream {
    let (ig, tyg, where_clause) = item.generics.split_for_impl();
    let ident = &item.ident;
    let source = source_fn(item, cases, eva);
    quote! {
        #[automatically_derived]
        impl #ig ::core::error::Error for #ident #tyg #where_clause {
            #source
        }
    }
}

fn source_fn(item: &Item, cases: &[Case], eva: &syn::Path) -> TokenStream {
    if cases.iter().all(|case| case.source.is_none()) {
        return quote! {};
    }

    let source = local("source");
    let arms = cases.iter().map(|case| match case.source {
        Some((index, field)) => {
            let pat = case.bind_one(index, &source);
            // Method-call autoderef reaches `dyn Error` inside boxes; spanned so that a field
            // that is not an error is reported at its type.
            let dyn_error = quote_spanned! {field.ty.span()=> {
                use #eva::_priv::AsDynError as _;
                #source.as_dyn_error()
            }};
            (pat, quote! { ::core::option::Option::Some(#dyn_error) })
        }
        None => (case.wildcard(), quote! { ::core::option::Option::None }),
    });
    let body = match &item.kind {
        Kind::Struct(_) => {
            let (pat, value) = arms.into_iter().next().expect("a struct is one case");
            quote! {
                let #pat = self;
                #value
            }
        }
        Kind::Enum(_) => {
            let (pats, values): (Vec<_>, Vec<_>) = arms.unzip();
            let cfgs = cases.iter().map(|case| &case.cfg);
            quote! {
                match self {
                    #(#cfgs #pats => #values,)*
                }
            }
        }
    };

    quote! {
        fn source(&self) -> ::core::option::Option<&(dyn ::core::error::Error + 'static)> {
            #body
        }
    }
}
