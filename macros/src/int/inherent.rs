use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};

use super::{
    Int,
    ranges::{Range, literal},
    variant,
};

impl Int {
    pub(super) fn inherent(&self) -> TokenStream {
        let name = &self.item.ident;
        let consts = self.consts();
        let constructors = self.constructors();
        let nth = self.nth();
        let slices = self.slices();
        quote! {
            impl #name {
                #consts
                #constructors
                #nth
                #slices
            }
        }
    }

    fn consts(&self) -> TokenStream {
        let vis = &self.item.vis;
        let repr = self.args.repr;
        let ranges = self.args.ranges.iter().map(|range| {
            let (start, end) = (literal(range.start), literal(range.end));
            quote!(#start..=#end)
        });
        let ranges_len = self.args.ranges.iter().count();
        let values = self.args.ranges.values().collect::<Vec<_>>();
        let len = values.len();
        let variants = values.iter().map(|&value| variant(value));
        let values = values.iter().map(|&value| literal(value));

        quote! {
            /// The allowed values as inclusive ranges: ascending, with overlapping and adjacent
            /// ranges merged.
            #vis const RANGES: [::core::ops::RangeInclusive<#repr>; #ranges_len] = [#(#ranges),*];
            /// Every allowed value, ascending and without duplicates.
            #vis const VALUES: [#repr; #len] = [#(#values),*];
            /// Every variant, in the order of [`Self::VALUES`]: `Self::VARIANTS[i].nth() == i`.
            #vis const VARIANTS: [Self; #len] = [#(Self::#variants),*];
        }
    }

    fn constructors(&self) -> TokenStream {
        let vis = &self.item.vis;
        let repr = self.args.repr;
        let patterns = self.args.ranges.iter().map(Range::pattern);
        quote_spanned! {Span::mixed_site()=>
            /// The variant for `value`, if it is one of [`Self::VALUES`].
            #vis const fn new(value: #repr) -> ::core::option::Option<Self> {
                match value {
                    #(#patterns)|* => ::core::option::Option::Some(unsafe { Self::new_unchecked(value) }),
                    _ => ::core::option::Option::None,
                }
            }

            /// The variant for `value`, unchecked.
            ///
            /// # Safety
            ///
            /// `value` must be one of [`Self::VALUES`].
            #vis const unsafe fn new_unchecked(value: #repr) -> Self {
                unsafe { ::core::mem::transmute::<#repr, Self>(value) }
            }

            /// The value of `self`.
            #vis const fn into_inner(self) -> #repr {
                self as #repr
            }
        }
    }

    /// Values stop at `i128::MAX`, so `as i128` is lossless, and a range holds at most
    /// `MAX_VALUES` values, so the arithmetic cannot overflow.
    fn nth(&self) -> TokenStream {
        let vis = &self.item.vis;
        let mut before = 0_usize;
        let mut arms = TokenStream::new();
        for range in self.args.ranges.iter() {
            let pattern = range.pattern();
            let start = literal(range.start);
            arms.extend(quote_spanned! {Span::mixed_site()=>
                #pattern => (__value - #start) as ::core::primitive::usize + #before,
            });
            before += (range.end - range.start) as usize + 1;
        }

        quote_spanned! {Span::mixed_site()=>
            /// The index of `self` in [`Self::VARIANTS`].
            #vis const fn nth(self) -> ::core::primitive::usize {
                let __value = self as ::core::primitive::i128;
                match __value {
                    #arms
                    _ => ::core::unreachable!(),
                }
            }
        }
    }

    // `#[repr(<repr>)]` gives `Self` the layout of the repr.
    fn slices(&self) -> TokenStream {
        let vis = &self.item.vis;
        let repr = self.args.repr;
        quote_spanned! {Span::mixed_site()=>
            /// Views `slice` as a slice of values.
            #vis const fn as_repr_slice(slice: &[Self]) -> &[#repr] {
                unsafe { ::core::slice::from_raw_parts(slice.as_ptr().cast(), slice.len()) }
            }

            /// Views `slice` as a mutable slice of values.
            ///
            /// # Safety
            ///
            /// Only values from [`Self::VALUES`] may be written through the returned slice.
            #vis const unsafe fn as_repr_slice_mut(slice: &mut [Self]) -> &mut [#repr] {
                unsafe { ::core::slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), slice.len()) }
            }
        }
    }
}
