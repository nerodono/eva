use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote};

use super::Args;

/// A qualified, explicit trace replaces the default. Aliased attributes need `not(trace)`:
/// a procedural macro cannot resolve imports to discover which macro an alias names.
pub(super) fn attribute(args: &Args, item: &syn::ItemFn) -> TokenStream {
    let eva = &args.eva;
    let reexport: syn::Path = syn::parse_quote!(#eva::_priv::fastrace::trace);
    let explicit = item.attrs.iter().any(|attr| {
        let path = attr.path();
        let segments: Vec<_> = path.segments.iter().map(|s| &s.ident).collect();
        matches!(segments.as_slice(), [krate, name] if *krate == "fastrace" && *name == "trace")
            || segments
                .into_iter()
                .eq(reexport.segments.iter().map(|s| &s.ident))
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
