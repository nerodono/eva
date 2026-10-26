use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote};
use syn::{Token, ext::IdentExt as _, parse::ParseStream, punctuated::Punctuated};

use heck::{ToKebabCase, ToLowerCamelCase, ToPascalCase, ToSnakeCase};

use crate::utils::{self, maybe};

use self::{args::Args, cases::Case};

mod args;
mod cases;
mod convert;
mod display;

/// Attributes that only the derives added by `expand` make valid.
const DERIVE_ATTRS: [&str; 5] = ["display", "from", "source", "serde", "schemars"];

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let (parsed, args) = utils::both(syn::parse2::<Item>(item.clone()), syn::parse2::<Args>(args))?;
    let (cases, ()) = utils::both(cases::collect(&parsed, args.error), args.check(&parsed))?;
    let display = display::impl_display(&parsed, &cases, args.display.as_ref(), args.error)?;

    let eva = &args.eva;
    let derives = args.derives.attrs(eva, &parsed.attrs);
    let error = maybe(args.error, || convert::error_impl(&parsed, &cases, eva));
    let from = convert::from_impls(&parsed, &cases);
    let as_static_str = match &parsed.kind {
        Kind::Enum(_) if args.as_static_str() => as_static_str(&parsed, &cases, eva),
        _ => quote! {},
    };

    Ok(quote! {
        #derives
        #[derive(#eva::_priv::RastGawno)]
        #item

        #error
        #from
        #as_static_str
        #display
    })
}

/// The item without [`DERIVE_ATTRS`], so its name stays resolvable after an error.
pub fn fallback(item: TokenStream) -> TokenStream {
    let Ok(mut parsed) = syn::parse2::<syn::Item>(item.clone()) else {
        return item;
    };
    let strip_fields = |fields: &mut syn::Fields| {
        fields
            .iter_mut()
            .for_each(|f| strip_derive_attrs(&mut f.attrs));
    };
    match &mut parsed {
        syn::Item::Struct(s) => {
            strip_derive_attrs(&mut s.attrs);
            strip_fields(&mut s.fields);
        }
        syn::Item::Enum(e) => {
            strip_derive_attrs(&mut e.attrs);
            for var in &mut e.variants {
                strip_derive_attrs(&mut var.attrs);
                strip_fields(&mut var.fields);
            }
        }
        _ => {}
    }
    parsed.into_token_stream()
}

fn strip_derive_attrs(attrs: &mut Vec<syn::Attribute>) {
    attrs.retain(|attr| !DERIVE_ATTRS.iter().any(|a| attr.path().is_ident(a)));
}

fn as_static_str(item: &Item, cases: &[Case], eva: &syn::Path) -> TokenStream {
    let convert = syn::Ident::new("convert_case", Span::mixed_site());
    let letter_case = quote! { #eva::generic::Case };
    let some = quote! { ::core::option::Option::Some };
    let arms = cases.iter().map(|case| {
        let cfg = &case.cfg;
        let pat = case.wildcard();
        let var = case.variant.expect("enum cases are variants");
        let name = var.ident.unraw().to_string();
        let snake = name.to_snake_case();
        let kebab = name.to_kebab_case();
        let pascal = name.to_pascal_case();
        let camel = name.to_lower_camel_case();

        quote! {
            #cfg
            #pat => match #convert {
                #some(#letter_case::Snake) => #snake,
                #some(#letter_case::Kebab) => #kebab,
                #some(#letter_case::Pascal) => #pascal,
                #some(#letter_case::Camel) => #camel,
                ::core::option::Option::None => #name,
            },
        }
    });
    let (ig, tyg, where_clause) = item.generics.split_for_impl();
    let ident = &item.ident;

    quote! {
        impl #ig #ident #tyg #where_clause {
            /// The variant name as written, or converted to `convert_case`.
            pub const fn as_static_str(
                &self,
                #convert: ::core::option::Option<#letter_case>,
            ) -> &'static ::core::primitive::str {
                match *self {
                    #(#arms)*
                }
            }
        }
    }
}

enum Kind {
    Struct(syn::Fields),
    Enum(Punctuated<syn::Variant, Token![,]>),
}

struct Item {
    attrs: Vec<syn::Attribute>,
    ident: syn::Ident,
    generics: syn::Generics,
    kind: Kind,
}

impl syn::parse::Parse for Item {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (attrs, ident, generics, kind) = match input.parse()? {
            syn::Item::Struct(s) => (s.attrs, s.ident, s.generics, Kind::Struct(s.fields)),
            syn::Item::Enum(e) => (e.attrs, e.ident, e.generics, Kind::Enum(e.variants)),
            other => {
                let msg = "only structs and enums are supported";
                return Err(syn::Error::new_spanned(other, msg));
            }
        };
        Ok(Self {
            attrs,
            ident,
            generics,
            kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::{TokenStream, TokenTree};
    use quote::quote;

    fn mentions_std(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|tt| match tt {
            TokenTree::Ident(ident) => ident == "std",
            TokenTree::Group(group) => mentions_std(group.stream()),
            _ => false,
        })
    }

    /// Generated code must compile in `#![no_std]` crates.
    #[test]
    fn generated_code_is_core_only() {
        let cases = [
            (
                quote! { error, display("failed") },
                quote! {
                    enum Failure<T> {
                        #[display("inner: {_0}")]
                        Inner(#[from] #[source] Inner),
                        #[display(name)]
                        Named { code: T },
                        Defaulted,
                    }
                },
            ),
            (
                quote! { error, display("wrapped: {_0}") },
                quote! { struct Wrapped(#[from] #[source] Inner); },
            ),
        ];
        for (args, item) in cases {
            let out = super::expand(args, item).unwrap();
            assert!(!mentions_std(out.clone()), "{out}");
        }
    }
}
