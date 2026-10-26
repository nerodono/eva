use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};

/// Accumulates independent errors so one expansion reports all of them.
#[derive(Default)]
pub struct Errors(Option<syn::Error>);

impl Errors {
    pub fn push(&mut self, error: syn::Error) {
        match &mut self.0 {
            Some(all) => all.combine(error),
            None => self.0 = Some(error),
        }
    }

    /// Records the error of `result`, if any, and returns its value.
    pub fn ok<T>(&mut self, result: syn::Result<T>) -> Option<T> {
        result.map_err(|e| self.push(e)).ok()
    }

    pub fn finish(self) -> syn::Result<()> {
        self.0.map_or(Ok(()), Err)
    }
}

/// Both values, or the errors of both.
pub fn both<A, B>(a: syn::Result<A>, b: syn::Result<B>) -> syn::Result<(A, B)> {
    match (a, b) {
        (Ok(a), Ok(b)) => Ok((a, b)),
        (Err(mut a), Err(b)) => {
            a.combine(b);
            Err(a)
        }
        (Err(e), Ok(_)) | (Ok(_), Err(e)) => Err(e),
    }
}

pub fn maybe<F>(b: bool, f: F) -> TokenStream
where
    F: FnOnce() -> TokenStream,
{
    if b {
        f()
    } else {
        quote! {}
    }
}

/// `__<name>` for a generated binding: `mixed_site` does not hide user consts and unit structs,
/// which would turn a plain name into a pattern (public method parameters stay plain for rustdoc).
pub fn local(name: &str) -> syn::Ident {
    format_ident!("__{name}", span = Span::mixed_site())
}

/// serde and schemars take their crate path as a string, which `$crate` has no spelling for.
pub fn is_dollar_crate(path: &syn::Path) -> bool {
    path.segments
        .first()
        .is_some_and(|segment| segment.ident == "$crate")
}

pub fn join(p: &syn::Path, next: &str) -> syn::Path {
    let ident = quote::format_ident!("{next}");
    syn::parse_quote!(#p :: #ident)
}
