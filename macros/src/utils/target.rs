//! Shared impl scaffolding; callers supply the value representation and schema body.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

use super::{local, maybe};
use crate::opts::CommonTraits;

type Split<'a> = (
    syn::ImplGenerics<'a>,
    TokenStream,
    Option<&'a syn::WhereClause>,
);

/// An annotated type, including any bounds added by its macro.
pub struct Target<'a> {
    pub ident: &'a syn::Ident,
    pub generics: syn::Generics,
}

impl<'a> Target<'a> {
    pub fn new(ident: &'a syn::Ident, generics: &syn::Generics) -> Self {
        Self {
            ident,
            generics: generics.clone(),
        }
    }

    pub fn split(&self) -> Split<'_> {
        let ident = self.ident;
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        (impl_generics, quote! { #ident #ty_generics }, where_clause)
    }

    /// Trait implementations share the target's bounds and generated-code lint policy.
    pub fn implement(&self, trait_path: TokenStream, body: TokenStream) -> TokenStream {
        let (generics, ty, where_clause) = self.split();
        quote! {
            #[automatically_derived]
            impl #generics #trait_path for #ty #where_clause { #body }
        }
    }

    /// Formatting with a caller-supplied body, also used by `#[data(display(...))]`.
    pub fn format(&self, trait_name: &str, body: TokenStream) -> TokenStream {
        let name = format_ident!("{trait_name}");
        let formatter = local("formatter");
        self.implement(
            quote!(::core::fmt::#name),
            quote! {
                #[inline]
                fn fmt(&self, #formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #body
                }
            },
        )
    }

    /// Hashing and total ordering through the same borrowed representation.
    pub fn hash_and_ord(&self, value: impl Fn(TokenStream) -> TokenStream) -> TokenStream {
        let own = value(quote!(self));
        let other = local("other");
        let rhs = value(quote!(#other));
        let state = local("state");
        [
            self.implement(quote!(::core::hash::Hash), quote! {
                #[inline]
                fn hash<__H: ::core::hash::Hasher>(&self, #state: &mut __H) {
                    ::core::hash::Hash::hash(#own, #state)
                }
            }),
            self.implement(quote!(::core::cmp::PartialOrd), quote! {
                #[inline]
                fn partial_cmp(&self, #other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                    ::core::option::Option::Some(::core::cmp::Ord::cmp(self, #other))
                }
            }),
            self.implement(quote!(::core::cmp::Ord), quote! {
                #[inline]
                fn cmp(&self, #other: &Self) -> ::core::cmp::Ordering {
                    ::core::cmp::Ord::cmp(#own, #rhs)
                }
            }),
        ].into_iter().collect()
    }

    pub fn with_lifetime(&self, lifetime: &syn::Lifetime) -> syn::Generics {
        let mut generics = self.generics.clone();
        generics
            .params
            .insert(0, syn::LifetimeParam::new(lifetime.clone()).into());
        generics
    }

    /// `value` borrows an `inner`; deserialization delegates to the supplied function.
    pub fn value_traits(
        &self,
        traits: &CommonTraits,
        display: bool,
        eva: &syn::Path,
        inner: TokenStream,
        value: TokenStream,
        deserialize: TokenStream,
    ) -> TokenStream {
        let formatter = local("formatter");
        let fmt = [(traits.debug, "Debug"), (display, "Display")]
            .into_iter()
            .map(|(on, name)| {
                maybe(on, || {
                    let fmt_trait = format_ident!("{name}");
                    self.format(
                        name,
                        quote! {
                            <#inner as ::core::fmt::#fmt_trait>::fmt(#value, #formatter)
                        },
                    )
                })
            });
        let serde = quote!(#eva::_priv::serde);
        let serializer = local("serializer");
        let serialize = maybe(traits.serialize, || {
            self.implement(
                quote!(#serde::Serialize),
                quote! {
                    #[inline]
                    fn serialize<__S: #serde::Serializer>(
                        &self, #serializer: __S,
                    ) -> ::core::result::Result<__S::Ok, __S::Error> {
                        <#inner as #serde::Serialize>::serialize(#value, #serializer)
                    }
                },
            )
        });
        let deserialize = maybe(traits.deserialize, || {
            let lifetime = syn::Lifetime::new("'__de", Span::mixed_site());
            let generics = self.with_lifetime(&lifetime);
            let (impl_generics, ..) = generics.split_for_impl();
            let (_, ty, where_clause) = self.split();
            let deserializer = local("deserializer");
            quote! {
                #[automatically_derived]
                impl #impl_generics #serde::Deserialize<#lifetime> for #ty #where_clause {
                    #[inline]
                    fn deserialize<__D: #serde::Deserializer<#lifetime>>(
                        #deserializer: __D,
                    ) -> ::core::result::Result<Self, __D::Error> {
                        #deserialize(#deserializer)
                    }
                }
            }
        });
        quote! { #(#fmt)* #serialize #deserialize }
    }

    /// Generic instantiations need distinct schema ids; plain types keep their module path.
    pub fn json_schema(&self, eva: &syn::Path, body: TokenStream) -> TokenStream {
        let name = self.ident.to_string();
        let id = if self.generics.params.is_empty() {
            quote! { ::core::concat!(::core::module_path!(), "::", #name) }
        } else {
            quote! { ::core::any::type_name::<Self>() }
        };
        let schemars = quote!(#eva::_priv::schemars);
        let generator = local("generator");
        self.implement(quote!(#schemars::JsonSchema), quote! {
                fn schema_name() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(#name)
                }

                fn schema_id() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(#id)
                }

                fn json_schema(#generator: &mut #schemars::SchemaGenerator) -> #schemars::Schema {
                    #body
                }
        })
    }
}
