//! Shared compile-time machinery for Eva's procedural macros.
pub mod docs;
pub mod opts;
mod paths;
pub mod utils;
pub use paths::{resolve_crate, resolve_crates};
#[cfg(feature = "auto-impl")]
pub mod auto_impl;
#[cfg(feature = "data")]
pub mod data_options;

use proc_macro2::TokenStream;
pub type Expand = fn(TokenStream, TokenStream) -> syn::Result<TokenStream>;

/// Keep the annotated name available when reporting errors, avoiding cascaded diagnostics.
pub fn run(
    args: TokenStream,
    item: TokenStream,
    expand: Expand,
    fallback: fn(TokenStream) -> TokenStream,
) -> TokenStream {
    match expand(args, item.clone()) {
        Ok(expanded) => expanded,
        Err(error) => {
            let mut out = fallback(item);
            out.extend(error.to_compile_error());
            out
        }
    }
}
