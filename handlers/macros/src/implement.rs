use std::collections::{HashMap, HashSet};

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream, Parser},
    spanned::Spanned,
};

struct Options {
    export: syn::Path,
    generics: syn::Generics,
    target: syn::Type,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let export = input.call(syn::Path::parse_mod_style)?;
        input.parse::<Token![,]>()?;
        let mut generics = if input.parse::<Option<Token![for]>>()?.is_some() {
            if !input.peek(Token![<]) {
                return Err(input.error("expected generic parameters after `for`"));
            }
            input.parse::<syn::Generics>()?
        } else {
            syn::Generics::default()
        };
        for param in &generics.params {
            let default = match param {
                syn::GenericParam::Type(param) => param.eq_token,
                syn::GenericParam::Const(param) => param.eq_token,
                syn::GenericParam::Lifetime(_) => None,
            };
            if let Some(token) = default {
                return Err(syn::Error::new(
                    token.span,
                    "impl parameters cannot have defaults",
                ));
            }
        }
        let target = input.parse()?;
        generics.where_clause = input.parse()?;
        input.parse::<Option<Token![,]>>()?;
        Ok(Self {
            export,
            generics,
            target,
        })
    }
}

/// Only the rule delimiters are parsed; matchers and transcribers remain opaque.
fn paste_rules(input: ParseStream, eva: &syn::Path) -> syn::Result<TokenStream> {
    let mut rules = TokenStream::new();
    while !input.is_empty() {
        let matcher: Group = input.parse()?;
        input.parse::<Token![=>]>()?;
        let body = input.parse::<Group>()?.stream();
        rules.extend(quote! {
            #matcher => { #eva::_priv::paste! { #body } };
        });
        if !input.is_empty() {
            input.parse::<Token![;]>()?;
        }
    }
    Ok(rules)
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let Options {
        export,
        generics,
        target,
    } = syn::parse2(args)?;
    let mut template = syn::parse2::<syn::ItemMacro>(item)?;
    if !template.mac.path.is_ident("macro_rules") || template.ident.is_none() {
        return Err(syn::Error::new(
            template.span(),
            "expected a named macro_rules! template",
        ));
    }
    let mut attrs = Vec::new();
    let mut extra = None;
    for attr in std::mem::take(&mut template.attrs) {
        if attr.path().is_ident("macro_export") {
            return Err(syn::Error::new(
                attr.span(),
                "implement templates are local and cannot be exported",
            ));
        }
        if attr.path().is_ident("extra") {
            if extra.is_some() {
                return Err(syn::Error::new(attr.span(), "duplicate extra attribute"));
            }
            let names = attr.parse_args_with(crate::utils::comma_list::<syn::Ident>)?;
            let mut seen = HashSet::new();
            extra = Some(
                names
                    .into_iter()
                    .map(|name| {
                        if !seen.insert(name.to_string()) {
                            return Err(syn::Error::new(name.span(), "duplicate extra argument"));
                        }
                        match name.to_string().as_str() {
                            "flags" => Ok(quote!([$($flag),*])),
                            "lifetimes" => Ok(quote!([$($lt),*])),
                            "ok" => Ok(quote!($Ok)),
                            _ => Err(syn::Error::new(
                                name.span(),
                                "expected flags, lifetimes, or ok",
                            )),
                        }
                    })
                    .collect::<syn::Result<Vec<_>>>()?,
            );
        } else {
            attrs.push(attr);
        }
    }
    let extra = extra.unwrap_or_default();
    let eva = crate::runtime();
    template.mac.tokens =
        (|input: ParseStream| paste_rules(input, &eva)).parse2(template.mac.tokens)?;
    let name = template.ident.as_ref().unwrap();
    let mut metadata = format_ident!("_{}", name);
    // The alias must not shadow the first segment of its own import path.
    while export.segments[0].ident == metadata {
        metadata = format_ident!("_{}", metadata);
    }
    let callback = format_ident!("__eva_implement_{}", name);
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    // A block keeps both macros local without changing self::/super:: paths.
    // The receiver and the `self` passed to the template share an expansion.
    Ok(quote! {
        #(#attrs)*
        const _: () = {
            // Resolve metadata outside the template's scope. Its original name
            // remains available for recursion, even when it matches the export.
            use #export as #metadata;
            const _: () = {
                #[allow(unused_macros)]
                #template

                macro_rules! #callback {
                    ($($accessor:ident {
                        trait: $Trait:path,
                        client: $Client:path,
                        methods: {$($method:ident {
                            flags: [$($flag:ident),*],
                            lifetimes: [$($lt:lifetime),*],
                            args: $Args:ty,
                            ok: $Ok:ty,
                            err: $Err:ty,
                            output: $Output:ty,
                            docs: [$($doc:literal),*],
                            $($method_extra:tt)*
                        })*},
                        $($extra:tt)*
                    })*) => {
                        #eva::_priv::implement_impls! {
                            $(impl #impl_generics $Trait for #target #where_clause {
                                $(
                                    $(#[doc = $doc])*
                                    fn $method<$($lt),*>(&mut self)
                                        -> impl #eva::_priv::CallStep<$Args, Output = $Output>
                                    {
                                        #name!(self, $accessor, $method, $Args #(, #extra)*)
                                    }
                                )*
                            })*
                        }
                    };
                }
                #metadata!(#callback);
            };
        };
    })
}

/// Metadata arrives after the attribute has expanded. Rename method lifetimes here
/// if they shadow an impl parameter, including inside the template's opaque Args.
/// No automatically_derived: the template contains user code and should retain its lints.
pub fn impls(input: TokenStream) -> syn::Result<TokenStream> {
    let mut file = syn::parse2::<syn::File>(input)?;
    for item in &mut file.items {
        let syn::Item::Impl(item) = item else {
            return Err(syn::Error::new(item.span(), "expected an impl"));
        };
        let outer: HashSet<_> = item
            .generics
            .lifetimes()
            .map(|lt| lt.lifetime.ident.to_string())
            .collect();
        let mut used = HashSet::new();
        map_lifetimes(quote!(#item), &mut |ident| {
            used.insert(ident.to_string());
        });
        for method in &mut item.items {
            let syn::ImplItem::Fn(method) = method else {
                continue;
            };
            let mut rename = HashMap::new();
            for param in method.sig.generics.lifetimes() {
                let old = param.lifetime.ident.to_string();
                if outer.contains(&old) {
                    let mut new = format!("__eva_{old}");
                    while !used.insert(new.clone()) {
                        new.push('_');
                    }
                    rename.insert(old, new);
                }
            }
            if !rename.is_empty() {
                // Only the generated signature and template invocation exist yet.
                // Walking tokens also reaches the opaque Args fragment in that invocation.
                *method = syn::parse2(map_lifetimes(quote!(#method), &mut |ident| {
                    if let Some(name) = rename.get(&ident.to_string()) {
                        *ident = syn::Ident::new(name, ident.span());
                    }
                }))?;
            }
        }
    }
    Ok(quote!(#file))
}

fn map_lifetimes(tokens: TokenStream, map: &mut impl FnMut(&mut syn::Ident)) -> TokenStream {
    let mut apostrophe = false;
    tokens
        .into_iter()
        .map(|token| {
            let token = match token {
                TokenTree::Group(group) => {
                    let mut renamed =
                        Group::new(group.delimiter(), map_lifetimes(group.stream(), map));
                    renamed.set_span(group.span());
                    TokenTree::Group(renamed)
                }
                TokenTree::Ident(mut ident) if apostrophe => {
                    map(&mut ident);
                    TokenTree::Ident(ident)
                }
                token => token,
            };
            apostrophe = matches!(&token, TokenTree::Punct(p) if p.as_char() == '\'');
            token
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_headers_are_rejected() {
        for header in [
            quote!(endpoints Backend),
            quote!(endpoints, for Backend),
            quote!(endpoints, for<T = ()> Backend<T>),
            quote!(endpoints, for<const N: usize = 1> Backend<N>),
            quote!(endpoints, Backend, unexpected),
        ] {
            assert!(syn::parse2::<Options>(header).is_err());
        }
    }

    #[test]
    fn templates_must_be_local_macro_rules() {
        for template in [
            quote!(
                #[macro_export]
                macro_rules! bind {}
            ),
            quote!(something_else! {}),
        ] {
            assert!(expand(quote!(endpoints, Backend), template).is_err());
        }
    }

    #[test]
    fn unknown_or_repeated_extras_are_rejected() {
        for attrs in [
            quote!(#[extra(unknown)]),
            quote!(#[extra(flags, flags)]),
            quote!(#[extra(flags)] #[extra(ok)]),
        ] {
            assert!(
                expand(
                    quote!(endpoints, Backend),
                    quote!(#attrs macro_rules! bind {}),
                )
                .is_err()
            );
        }
    }
}
