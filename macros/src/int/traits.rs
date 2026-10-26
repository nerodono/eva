use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};

use super::{Int, repr::Bound};
use crate::{
    docs,
    utils::{self, maybe},
};

impl Int {
    pub(super) fn traits(&self) -> TokenStream {
        let traits = &self.args.traits;
        let copy_and_cmp = self.copy_and_cmp();
        let debug = maybe(traits.debug, || self.fmt("Debug"));
        let display = maybe(self.args.display, || self.fmt("Display"));
        let serialize = maybe(traits.serialize, || self.serialize());
        let deserialize = maybe(traits.deserialize, || self.deserialize());
        let json_schema = maybe(traits.json_schema, || self.json_schema());
        let rand = self.rand();
        let integer = self.integer();

        quote! {
            #copy_and_cmp
            #debug
            #display
            #serialize
            #deserialize
            #json_schema
            #rand
            #integer
        }
    }

    fn copy_and_cmp(&self) -> TokenStream {
        let name = &self.item.ident;
        let repr = self.args.repr;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl ::core::marker::Copy for #name {}

            #[automatically_derived]
            impl ::core::clone::Clone for #name {
                fn clone(&self) -> Self {
                    *self
                }
            }

            #[automatically_derived]
            impl ::core::hash::Hash for #name {
                fn hash<__H: ::core::hash::Hasher>(&self, __state: &mut __H) {
                    ::core::hash::Hash::hash(&Self::into_inner(*self), __state)
                }
            }

            #[automatically_derived]
            impl ::core::cmp::PartialEq<#repr> for #name {
                fn eq(&self, __other: &#repr) -> ::core::primitive::bool {
                    Self::into_inner(*self) == *__other
                }
            }

            #[automatically_derived]
            impl ::core::cmp::PartialOrd for #name {
                fn partial_cmp(&self, __other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                    ::core::option::Option::Some(::core::cmp::Ord::cmp(self, __other))
                }
            }

            #[automatically_derived]
            impl ::core::cmp::Ord for #name {
                fn cmp(&self, __other: &Self) -> ::core::cmp::Ordering {
                    ::core::cmp::Ord::cmp(&Self::into_inner(*self), &Self::into_inner(*__other))
                }
            }

            #[automatically_derived]
            impl ::core::convert::From<#name> for #repr {
                fn from(__value: #name) -> Self {
                    #name::into_inner(__value)
                }
            }
        }
    }

    /// `Debug` or `Display`, as the number.
    fn fmt(&self, trait_name: &str) -> TokenStream {
        let name = &self.item.ident;
        let repr = self.args.repr;
        let fmt_trait = format_ident!("{trait_name}");
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl ::core::fmt::#fmt_trait for #name {
                fn fmt(&self, __f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    <#repr as ::core::fmt::#fmt_trait>::fmt(&Self::into_inner(*self), __f)
                }
            }
        }
    }

    fn serialize(&self) -> TokenStream {
        let name = &self.item.ident;
        let repr = self.args.repr;
        let eva = &self.args.eva;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #eva::_priv::serde::Serialize for #name {
                fn serialize<S: #eva::_priv::serde::Serializer>(
                    &self,
                    __serializer: S,
                ) -> ::core::result::Result<S::Ok, S::Error> {
                    <#repr as #eva::_priv::serde::Serialize>::serialize(&Self::into_inner(*self), __serializer)
                }
            }
        }
    }

    fn deserialize(&self) -> TokenStream {
        let name = &self.item.ident;
        let eva = &self.args.eva;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl<'de> #eva::_priv::serde::Deserialize<'de> for #name {
                fn deserialize<__D: #eva::_priv::serde::Deserializer<'de>>(
                    __deserializer: __D,
                ) -> ::core::result::Result<Self, __D::Error> {
                    #eva::int::deserialize(__deserializer)
                }
            }
        }
    }
    fn json_schema(&self) -> TokenStream {
        let name = &self.item.ident;
        let schema_name = name.to_string();
        let repr = self.args.repr;
        let schemars = {
            let eva = &self.args.eva;
            quote!(#eva::_priv::schemars)
        };
        let description = docs::schema_description(&self.item.attrs);
        let any_of = self.args.ranges.iter().map(|range| {
            let minimum = repr.schema_bound(range.start, Bound::Minimum);
            let maximum = repr.schema_bound(range.end, Bound::Maximum);
            quote!({ "type": "integer", "minimum": #minimum, "maximum": #maximum })
        });

        quote! {
            #[automatically_derived]
            impl #schemars::JsonSchema for #name {
                fn schema_name() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(#schema_name)
                }

                fn schema_id() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(::core::concat!(::core::module_path!(), "::", #schema_name))
                }

                fn json_schema(_: &mut #schemars::SchemaGenerator) -> #schemars::Schema {
                    #schemars::json_schema!({
                        #description
                        "anyOf": [#(#any_of),*]
                    })
                }
            }
        }
    }

    /// Uniform over the values.
    fn rand(&self) -> TokenStream {
        let name = &self.item.ident;
        let rand = {
            let eva = &self.args.eva;
            quote!(#eva::rand)
        };
        // Named after the enum so that it cannot shadow it.
        let rng = utils::local(&format!("Rng{name}"));
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #rand::distr::Distribution<#name> for #rand::distr::StandardUniform {
                fn sample<#rng: #rand::Rng + ?::core::marker::Sized>(&self, __rng: &mut #rng) -> #name {
                    let __index = <#rng as #rand::Rng>::random_range(__rng, 0..#name::VARIANTS.len());
                    #name::VARIANTS[__index]
                }
            }
        }
    }

    /// `eva::int::Integer` and `InfallibleInteger`, so the enum can be the field of an
    /// `#[int(newtype)]`.
    fn integer(&self) -> TokenStream {
        let name = &self.item.ident;
        let repr = self.args.repr;
        let eva = &self.args.eva;
        let expecting = format!("an integer in {}", self.args.ranges);
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #eva::int::Integer for #name {
                type Inner = Self;
                type Prim = #repr;

                #[inline]
                fn try_new(__prim: #repr) -> ::core::option::Option<Self> {
                    Self::new(__prim)
                }

                #[inline]
                unsafe fn new_unchecked(__prim: #repr) -> Self {
                    unsafe { Self::new_unchecked(__prim) }
                }

                #[inline]
                fn inner(&self) -> Self {
                    *self
                }

                #[inline]
                fn prim(&self) -> #repr {
                    Self::into_inner(*self)
                }

                fn expecting(__f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    ::core::fmt::Formatter::write_str(__f, #expecting)
                }
            }

            #[automatically_derived]
            impl #eva::int::InfallibleInteger for #name {
                #[inline]
                fn new(__inner: Self) -> Self {
                    __inner
                }
            }
        }
    }
}
