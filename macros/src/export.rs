use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use crate::{
    opts::{Key, Opts, Table},
    utils::{comma_list, unique_ident},
};

const OPTIONS: Table = Table {
    keys: &[Key::value::<syn::Ident>("name"), Key::list("flags")],
    not: &[],
};

struct Options {
    name: syn::Ident,
    flags: Vec<syn::Ident>,
}
impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let opts = Opts::parse(input, &OPTIONS)?;
        let flags = opts.list("flags", |input| {
            let list = comma_list::<syn::Ident>(input)?;
            let mut seen = HashSet::new();
            for flag in &list {
                if !seen.insert(flag.to_string()) {
                    return Err(syn::Error::new(flag.span(), "duplicate flag"));
                }
            }
            Ok(list)
        })?;
        Ok(Self {
            name: opts
                .value("name")
                .unwrap_or_else(|| syn::parse_quote!(for_each_endpoint)),
            flags: flags.unwrap_or_default(),
        })
    }
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let opts = syn::parse2::<Options>(args)?;
    let item = syn::parse2::<syn::ItemStruct>(item)?;
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            item.generics.span(),
            "export supplies the implementation parameter S",
        ));
    }
    let syn::Fields::Named(fields) = &item.fields else {
        return Err(syn::Error::new(
            item.span(),
            "expected named service fields",
        ));
    };
    let name = &item.ident;
    let vis = &item.vis;
    let attrs = &item.attrs;
    let features = format_ident!("{}Features", name);
    let export = unique_ident("export", name.span());
    let macro_name = opts.name;
    let mut bounds = Vec::new();
    let mut accessors = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut exported_chain = Vec::new();
    let mut local_chain = Vec::new();
    for field in &fields.named {
        let accessor = field.ident.as_ref().unwrap();
        let syn::Type::Path(ty) = &field.ty else {
            return Err(syn::Error::new(
                field.ty.span(),
                "expected a service module path",
            ));
        };
        if ty.qself.is_some()
            || ty
                .path
                .segments
                .iter()
                .any(|s| !matches!(s.arguments, syn::PathArguments::None))
        {
            return Err(syn::Error::new(
                ty.span(),
                "expected a service module path without type arguments",
            ));
        }
        let path = &ty.path;
        let exported = quote!($crate::#macro_name::#accessor);
        let import = unique_ident("export_service", accessor.span());
        imports.push(quote!(use #path as #import;));
        exports.push(quote! {
            pub mod #accessor {
                pub use super::super::#import::*;
            }
        });
        let field_attrs = &field.attrs;
        bounds.push(quote!(#path::Trait));
        accessors.push(quote! {
            #(#field_attrs)*
            pub fn #accessor(&mut self) -> #path::Client<&mut S> {
                #path::Client::new(&mut self.0)
            }
        });
        exported_chain.push((exported, accessor));
        local_chain.push((quote!(#path), accessor));
    }
    let invoke = chain(&exported_chain, quote!($($callback)::+));
    let check = unique_ident("export_flags", name.span());
    let check_invoke = chain(&local_chain, quote!(#check));
    let allowed = &opts.flags;
    Ok(quote! {
        #(#attrs)*
        #[repr(transparent)]
        #vis struct #name<S>(pub S);

        #vis trait #features: Send + Sync #(+ #bounds)* {}
        impl<S: Send + Sync #(+ #bounds)*> #features for S {}

        impl<S> #name<S> {
            pub const fn new(inner: S) -> Self { Self(inner) }
            pub fn into_inner(self) -> S { self.0 }
            pub fn inner_mut(&mut self) -> &mut S { &mut self.0 }
            pub fn mut_(&mut self) -> #name<&mut S> { #name(&mut self.0) }
        }
        impl<S: #features> #name<S> { #(#accessors)* }

        // Re-exporting the macro also exports this module in the type namespace.
        // Resolve service imports here, before metadata crosses a crate boundary.
        #(#imports)*
        #[doc(hidden)]
        #vis mod #macro_name { #(#exports)* }

        #[doc(hidden)]
        #[macro_export]
        macro_rules! #export {
            ($($callback:ident)::+) => {
                #invoke
            };
        }
        #vis use #export as #macro_name;

        macro_rules! #check {
            ($($accessor:ident {
                trait: $service_trait:path,
                client: $client:path,
                methods: {$($method:ident {
                    flags: [$($flag:ident),*],
                    $($method_rest:tt)*
                })*},
                $($rest:tt)*
            })*) => {
                ::eva::_priv::check_flags!([#(#allowed),*] $($($($flag)*)*)*);
            };
        }
        #check_invoke
    })
}

/// Invokes `callback` with the metadata of `services`, threaded through their macros.
fn chain(services: &[(TokenStream, &syn::Ident)], callback: TokenStream) -> TokenStream {
    let Some(((path, accessor), rest)) = services.split_first() else {
        return quote! { #callback! {} };
    };
    let rest = rest
        .iter()
        .map(|(path, accessor)| quote!(([#path] #accessor)));
    quote! { #path::for_each_endpoint! { @append [#path] [#accessor] [#callback] [] [#(#rest)*] } }
}
