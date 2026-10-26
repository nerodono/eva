use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

struct Options {
    name: syn::Ident,
}
impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        if !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            if key != "name" {
                return Err(syn::Error::new(
                    key.span(),
                    "expected `name = for_each_endpoint`",
                ));
            }
            input.parse::<Token![=]>()?;
            name = Some(input.parse()?);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self {
            name: name.unwrap_or_else(|| syn::parse_quote!(for_each_endpoint)),
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
    let export = super::service::unique("export", name);
    let macro_name = opts.name;
    let mut bounds = Vec::new();
    let mut accessors = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut steps = Vec::new();
    let mut first = None;
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
        let import = super::service::unique("export_service", accessor);
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
        if first.is_none() {
            first = Some((exported, accessor));
        } else {
            steps.push(quote! { ([#exported] #accessor #accessor) });
        }
    }
    let invoke = if let Some((path, accessor)) = first {
        quote! { #path::for_each_endpoint! { @append [#path] [#accessor #accessor] [$($callback)::+] [] [#(#steps)*] } }
    } else {
        quote! { $($callback)::+! {} }
    };
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
    })
}
