use std::hash::{DefaultHasher, Hash, Hasher};

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::{format_ident, quote};

mod fields;
mod target;

pub use fields::{construct, delimit, only_field, requires_struct};
pub use target::Target;

/// A comma-separated list, allowing a trailing comma.
pub fn comma_list<T: syn::parse::Parse>(input: syn::parse::ParseStream) -> syn::Result<Vec<T>> {
    syn::punctuated::Punctuated::<T, syn::Token![,]>::parse_terminated(input)
        .map(|list| list.into_iter().collect())
}

/// A helper name determined solely by its prefix and source coordinates.
pub fn unique_ident(prefix: &str, span: Span) -> syn::Ident {
    // Temporary hack until macros 2.0 gives public helpers module scope instead of
    // #[macro_export]'s crate-root namespace. Coordinates avoid global expansion state,
    // but may coincide in repeated macro_rules! expansions, which service/export do not
    // support. Hash the remapped file path only; declaration contents do not affect names.
    let mut path_hash = DefaultHasher::new();
    span.file().hash(&mut path_hash);
    let start = span.start();
    format_ident!(
        "__eva_{}_{}_{}_{:x}",
        prefix,
        start.line,
        start.column,
        path_hash.finish(),
        span = span,
    )
}

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

/// The first matching identifier, including inside token groups and macro arguments.
pub fn find_ident(tokens: TokenStream, name: &str) -> Option<Span> {
    tokens.into_iter().find_map(|token| match token {
        TokenTree::Ident(ident) if ident == name => Some(ident.span()),
        TokenTree::Group(group) => find_ident(group.stream(), name),
        _ => None,
    })
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
