use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};

use super::Args;

/// A qualified, explicit trace replaces the default. Aliased attributes need `not(trace)`:
/// a procedural macro cannot resolve imports to discover which macro an alias names.
pub(super) fn attribute(args: &Args, item: &syn::ItemFn) -> TokenStream {
    let eva = &args.eva;
    let reexport: syn::Path = syn::parse_quote!(#eva::_priv::fastrace::trace);
    let mut explicit_paths: Vec<syn::Path> =
        vec![syn::parse_quote!(fastrace::trace), reexport.clone()];
    for (package, root) in eva_macros_common::resolve_crates(&["eva-handlers", "eva"]) {
        explicit_paths.push(syn::parse_quote!(#root::_priv::fastrace::trace));
        if package == "eva" {
            explicit_paths.push(syn::parse_quote!(#root::handlers::_priv::fastrace::trace));
        }
    }
    if let Ok(root) = eva_macros_common::resolve_crate("fastrace") {
        explicit_paths.push(syn::parse_quote!(#root::trace));
    }
    let explicit = item.attrs.iter().any(|attr| {
        let path = attr.path();
        explicit_paths.iter().any(|known| {
            path.segments
                .iter()
                .map(|s| &s.ident)
                .eq(known.segments.iter().map(|s| &s.ident))
        })
    });
    if !args.trace || explicit {
        return quote! {};
    }
    let name = match &args.method {
        Some(path) => path.to_token_stream().to_string().replace(' ', ""),
        None => item.sig.ident.to_string(),
    };
    quote! {
        #[#reexport(crate = #eva::_priv::fastrace, name = #name)]
    }
}
