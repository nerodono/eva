use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

use super::{Int, repr::Bound};
use crate::{
    docs,
    utils::{self, Target, maybe},
};

impl Int {
    pub(super) fn traits(&self) -> TokenStream {
        let traits = &self.args.traits;
        let eva = &self.args.eva;
        let repr = self.args.repr;
        let target = Target::new(&self.item.ident, &self.item.generics);
        let hash_and_ord = target.hash_and_ord(|value| quote!(&Self::into_inner(*#value)));
        let conversions = self.conversions();
        let value_traits = target.value_traits(
            traits,
            self.args.display,
            eva,
            quote!(#repr),
            quote!(&Self::into_inner(*self)),
            quote!(#eva::int::deserialize),
        );
        let json_schema = maybe(traits.json_schema, || self.json_schema());
        let rand = self.rand();
        let integer = self.integer();

        quote! {
            #hash_and_ord
            #conversions
            #value_traits
            #json_schema
            #rand
            #integer
        }
    }

    fn conversions(&self) -> TokenStream {
        let name = &self.item.ident;
        let repr = self.args.repr;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl ::core::cmp::PartialEq<#repr> for #name {
                fn eq(&self, __other: &#repr) -> ::core::primitive::bool {
                    Self::into_inner(*self) == *__other
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

    fn json_schema(&self) -> TokenStream {
        let target = Target::new(&self.item.ident, &self.item.generics);
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

        target.json_schema(
            &self.args.eva,
            quote! {
                #schemars::json_schema!({
                    #description
                    "anyOf": [#(#any_of),*]
                })
            },
        )
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
