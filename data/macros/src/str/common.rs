//! The surface every kind shares, all through `Self::as_str`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::utils::local;

use super::{Target, args::Args};

pub fn impls(target: &Target, args: &Args) -> TokenStream {
    let eva = &args.eva;
    let mut out = TokenStream::new();
    out.extend(hash_and_cmp(target));
    out.extend(str_eq(target));
    out.extend(borrow(target));
    let as_str = super::as_str(target);
    out.extend(target.value_traits(
        &args.traits,
        args.display,
        eva,
        quote!(::core::primitive::str),
        quote!(#as_str(self)),
        quote!(#eva::_priv::deserialize_from_str),
    ));
    out
}

/// A newtype delegates to its field; a fixed string concatenates all field patterns.
pub fn pattern(target: &Target, fields: &syn::Fields, eva: &syn::Path) -> TokenStream {
    let field_tys = fields.iter().map(|field| &field.ty);
    let buf = local("buf");
    target.implement(
        quote!(#eva::str::HasPattern),
        quote! {
            #[inline]
            fn pat_into(#buf: &mut ::std::string::String) {
                #(<#field_tys as #eva::str::HasPattern>::pat_into(#buf);)*
            }
        },
    )
}

/// Consistent with `Borrow<str>`, so `&str` lookups in maps and sets find the value.
fn hash_and_cmp(target: &Target) -> TokenStream {
    let as_str = super::as_str(target);
    let other = local("other");
    [
        target.hash_and_ord(|value| quote!(#as_str(#value))),
        target.implement(
            quote!(::core::cmp::PartialEq),
            quote! {
                #[inline]
                fn eq(&self, #other: &Self) -> ::core::primitive::bool {
                    #as_str(self) == #as_str(#other)
                }
            },
        ),
        target.implement(quote!(::core::cmp::Eq), quote!()),
    ]
    .into_iter()
    .collect()
}

/// `==` with `str`, `&str` and `String`, in both directions; the reverse ones delegate.
fn str_eq(target: &Target) -> TokenStream {
    let lifetime = syn::Lifetime::new("'__str", Span::call_site());
    let with_lifetime = target.with_lifetime(&lifetime);
    let (lt_impl_generics, ..) = with_lifetime.split_for_impl();
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = super::as_str(target);
    let str = quote! { ::core::primitive::str };
    let string = quote! { ::std::string::String };
    let bool = quote! { ::core::primitive::bool };
    let other = local("other");

    [
        (quote!(#str), quote!(#other), quote!(self), &impl_generics),
        (
            quote!(&#lifetime #str),
            quote!(*#other),
            quote!(*self),
            &lt_impl_generics,
        ),
        (
            quote!(#string),
            quote!(#string::as_str(#other)),
            quote!(#string::as_str(self)),
            &impl_generics,
        ),
    ]
    .into_iter()
    .map(|(rhs, other_str, self_str, generics)| {
        quote! {
            #[automatically_derived]
            impl #generics ::core::cmp::PartialEq<#rhs> for #ty #where_clause {
                #[inline]
                fn eq(&self, #other: &#rhs) -> #bool {
                    #as_str(self) == #other_str
                }
            }

            #[automatically_derived]
            impl #generics ::core::cmp::PartialEq<#ty> for #rhs #where_clause {
                #[inline]
                fn eq(&self, #other: &#ty) -> #bool {
                    <#ty as ::core::cmp::PartialEq<#str>>::eq(#other, #self_str)
                }
            }
        }
    })
    .collect()
}

fn borrow(target: &Target) -> TokenStream {
    let as_str = super::as_str(target);
    let str = quote!(::core::primitive::str);
    [
        (quote!(::core::borrow::Borrow<#str>), "borrow", quote!()),
        (quote!(::core::convert::AsRef<#str>), "as_ref", quote!()),
        (
            quote!(::core::ops::Deref),
            "deref",
            quote!(type Target = #str;),
        ),
    ]
    .into_iter()
    .map(|(trait_path, method, associated)| {
        let method = format_ident!("{method}");
        target.implement(
            trait_path,
            quote! {
                #associated
                #[inline]
                fn #method(&self) -> &#str { #as_str(self) }
            },
        )
    })
    .collect()
}
