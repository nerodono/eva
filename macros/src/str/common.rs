//! The surface every kind shares, all through `Target::as_str`.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use crate::utils::{join, local, maybe};

use super::{Target, args::Args};

pub fn impls(target: &Target, args: &Args) -> TokenStream {
    let traits = &args.traits;
    let eva = &args.eva;
    let mut out = TokenStream::new();
    out.extend(hash_and_cmp(target));
    out.extend(str_eq(target));
    out.extend(borrow(target));
    out.extend(maybe(traits.debug, || fmt(target, "Debug")));
    out.extend(maybe(traits.display, || fmt(target, "Display")));
    out.extend(maybe(traits.serialize, || serialize(target, eva)));
    out.extend(maybe(traits.deserialize, || deserialize(target, eva)));
    out
}

/// Consistent with `Borrow<str>`, so `&str` lookups in maps and sets find the value.
fn hash_and_cmp(target: &Target) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = target.as_str();
    let state = local("state");
    let other = local("other");

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::hash::Hash for #ty #where_clause {
            #[inline]
            fn hash<__H: ::core::hash::Hasher>(&self, #state: &mut __H) {
                ::core::hash::Hash::hash(#as_str(self), #state)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialEq for #ty #where_clause {
            #[inline]
            fn eq(&self, #other: &Self) -> ::core::primitive::bool {
                #as_str(self) == #as_str(#other)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::Eq for #ty #where_clause {}

        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialOrd for #ty #where_clause {
            #[inline]
            fn partial_cmp(
                &self,
                #other: &Self,
            ) -> ::core::option::Option<::core::cmp::Ordering> {
                ::core::option::Option::Some(::core::cmp::Ord::cmp(self, #other))
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::Ord for #ty #where_clause {
            #[inline]
            fn cmp(&self, #other: &Self) -> ::core::cmp::Ordering {
                ::core::cmp::Ord::cmp(#as_str(self), #as_str(#other))
            }
        }
    }
}

/// `==` with `str`, `&str` and `String`, in both directions; the reverse ones delegate.
fn str_eq(target: &Target) -> TokenStream {
    let lifetime = syn::Lifetime::new("'__str", Span::call_site());
    let with_lifetime = target.with_lifetime(&lifetime);
    let (lt_impl_generics, ..) = with_lifetime.split_for_impl();
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = target.as_str();
    let str = quote! { ::core::primitive::str };
    let string = quote! { ::std::string::String };
    let bool = quote! { ::core::primitive::bool };
    let other = local("other");

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialEq<#str> for #ty #where_clause {
            #[inline]
            fn eq(&self, #other: &#str) -> #bool {
                #as_str(self) == #other
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialEq<#ty> for #str #where_clause {
            #[inline]
            fn eq(&self, #other: &#ty) -> #bool {
                <#ty as ::core::cmp::PartialEq<#str>>::eq(#other, self)
            }
        }

        #[automatically_derived]
        impl #lt_impl_generics ::core::cmp::PartialEq<&#lifetime #str> for #ty #where_clause {
            #[inline]
            fn eq(&self, #other: &&#lifetime #str) -> #bool {
                #as_str(self) == *#other
            }
        }

        #[automatically_derived]
        impl #lt_impl_generics ::core::cmp::PartialEq<#ty> for &#lifetime #str #where_clause {
            #[inline]
            fn eq(&self, #other: &#ty) -> #bool {
                <#ty as ::core::cmp::PartialEq<#str>>::eq(#other, *self)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialEq<#string> for #ty #where_clause {
            #[inline]
            fn eq(&self, #other: &#string) -> #bool {
                #as_str(self) == #string::as_str(#other)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::cmp::PartialEq<#ty> for #string #where_clause {
            #[inline]
            fn eq(&self, #other: &#ty) -> #bool {
                <#ty as ::core::cmp::PartialEq<#str>>::eq(#other, #string::as_str(self))
            }
        }
    }
}

fn borrow(target: &Target) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = target.as_str();
    let str = quote! { ::core::primitive::str };

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::borrow::Borrow<#str> for #ty #where_clause {
            #[inline]
            fn borrow(&self) -> &#str {
                #as_str(self)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::convert::AsRef<#str> for #ty #where_clause {
            #[inline]
            fn as_ref(&self) -> &#str {
                #as_str(self)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::ops::Deref for #ty #where_clause {
            type Target = #str;

            #[inline]
            fn deref(&self) -> &#str {
                #as_str(self)
            }
        }
    }
}

/// Delegates to `str`'s impl, so `Display` applies width, fill and precision and `Debug` quotes.
fn fmt(target: &Target, trait_name: &str) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = target.as_str();
    let trait_name = format_ident!("{trait_name}");
    let f = local("f");

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::fmt::#trait_name for #ty #where_clause {
            #[inline]
            fn fmt(&self, #f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                <::core::primitive::str as ::core::fmt::#trait_name>::fmt(#as_str(self), #f)
            }
        }
    }
}

fn serialize(target: &Target, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let as_str = target.as_str();
    let serde = join(&join(eva, "_priv"), "serde");
    let serializer = local("serializer");

    quote! {
        #[automatically_derived]
        impl #impl_generics #serde::Serialize for #ty #where_clause {
            #[inline]
            fn serialize<__S: #serde::Serializer>(
                &self,
                #serializer: __S,
            ) -> ::core::result::Result<__S::Ok, __S::Error> {
                #serde::Serializer::serialize_str(#serializer, #as_str(self))
            }
        }
    }
}

/// Through `FromStr`, from borrowed, owned or UTF-8 byte input.
fn deserialize(target: &Target, eva: &syn::Path) -> TokenStream {
    let lifetime = syn::Lifetime::new("'__de", Span::call_site());
    let with_lifetime = target.with_lifetime(&lifetime);
    let (impl_generics, ..) = with_lifetime.split_for_impl();
    let (_, ty, where_clause) = target.split();
    let serde = join(&join(eva, "_priv"), "serde");
    let deserializer = local("deserializer");

    quote! {
        #[automatically_derived]
        impl #impl_generics #serde::Deserialize<#lifetime> for #ty #where_clause {
            #[inline]
            fn deserialize<__D: #serde::Deserializer<#lifetime>>(
                #deserializer: __D,
            ) -> ::core::result::Result<Self, __D::Error> {
                #eva::_priv::deserialize_from_str(#deserializer)
            }
        }
    }
}
