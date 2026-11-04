use proc_macro2::TokenStream;
use quote::quote;

/// The `#[doc = ...]` values in order; `#[doc(hidden)]` and other forms carry no text.
pub fn exprs(attrs: &[syn::Attribute]) -> impl Iterator<Item = &syn::Expr> {
    attrs.iter().filter_map(|attr| match &attr.meta {
        syn::Meta::NameValue(nv) if nv.path.is_ident("doc") => Some(&nv.value),
        _ => None,
    })
}

/// The docs as one `&'static str` expression, `None` without docs; the lines are joined and
/// trimmed at runtime like the `JsonSchema` derive does, so `include_str!` docs work.
pub fn description(attrs: &[syn::Attribute]) -> Option<TokenStream> {
    let mut exprs = exprs(attrs);
    let first = exprs.next()?;
    Some(quote! {
        ::core::primitive::str::trim(::core::concat!(#first #(, "\n", #exprs)*))
    })
}

/// The `"description": ...,` entry of a `json_schema!` object, empty without docs.
pub fn schema_description(attrs: &[syn::Attribute]) -> TokenStream {
    match description(attrs) {
        Some(description) => quote! { "description": #description, },
        None => quote! {},
    }
}
