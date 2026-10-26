//! `Display` from `display(...)` on the type and `#[display(...)]` on enum variants.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Token,
    ext::IdentExt as _,
    parse::{Parse, ParseStream},
};

use heck::ToSnakeCase as _;

use crate::{
    docs,
    utils::{Errors, local},
};

use super::{Item, Kind, cases::Case};

/// `"fmt", args...`, `name` or `doc`.
pub enum DisplayArg {
    Fmt {
        fmt: syn::LitStr,
        args: Vec<syn::Expr>,
    },
    /// The snake_case variant name.
    Name(syn::Ident),
    /// The variant's doc comment.
    Doc(syn::Ident),
}

impl Parse for DisplayArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        const EXPECTED: &str = "expected a format string, `name` or `doc`";
        if input.peek(syn::LitStr) {
            return parse_fmt(input);
        }
        let mode = input
            .parse::<syn::Ident>()
            .map_err(|e| syn::Error::new(e.span(), EXPECTED))?;
        let display = match mode.to_string().as_str() {
            "name" => Self::Name(mode.clone()),
            "doc" => Self::Doc(mode.clone()),
            _ => return Err(syn::Error::new(mode.span(), EXPECTED)),
        };
        if !input.is_empty() {
            let rest: TokenStream = input.parse()?;
            let msg = format!("`{mode}` takes no arguments");
            return Err(syn::Error::new_spanned(rest, msg));
        }
        Ok(display)
    }
}

fn parse_fmt(input: ParseStream) -> syn::Result<DisplayArg> {
    let fmt = input.parse()?;
    let mut args = Vec::new();
    while !input.is_empty() {
        input.parse::<Token![,]>()?;
        if input.is_empty() {
            break;
        }
        args.push(input.parse()?);
    }
    Ok(DisplayArg::Fmt { fmt, args })
}

/// The `Display` impl, if the options and attributes ask for one.
pub fn impl_display(
    item: &Item,
    cases: &[Case],
    default: Option<&DisplayArg>,
    error: bool,
) -> syn::Result<TokenStream> {
    let f = local("formatter");
    let body = match &item.kind {
        Kind::Struct(_) => struct_body(&cases[0], default, &f)?,
        Kind::Enum(_) => enum_body(cases, default, error, &f)?,
    };
    let Some(body) = body else {
        return Ok(quote! {});
    };

    let (ig, tyg, where_clause) = item.generics.split_for_impl();
    let ident = &item.ident;
    Ok(quote! {
        #[automatically_derived]
        impl #ig ::core::fmt::Display for #ident #tyg #where_clause {
            #[inline]
            fn fmt(&self, #f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #body
            }
        }
    })
}

fn struct_body(
    case: &Case,
    display: Option<&DisplayArg>,
    f: &syn::Ident,
) -> syn::Result<Option<TokenStream>> {
    match display {
        None => Ok(None),
        Some(DisplayArg::Fmt { fmt, args }) => {
            let pat = case.bind_all();
            let write = write(f, fmt, args);
            Ok(Some(quote! {
                let #pat = self;
                #write
            }))
        }
        Some(DisplayArg::Name(mode) | DisplayArg::Doc(mode)) => {
            let msg = format!("`display({mode})` only applies to enums; use a format string");
            Err(syn::Error::new(mode.span(), msg))
        }
    }
}

/// Each variant uses its own `#[display]`, else the type-level `default`.
fn enum_body(
    cases: &[Case],
    default: Option<&DisplayArg>,
    error: bool,
    f: &syn::Ident,
) -> syn::Result<Option<TokenStream>> {
    if cases.is_empty() {
        // Uninhabited, so `error` alone warrants the impl.
        return Ok((default.is_some() || error).then(|| quote! { match *self {} }));
    }
    let overridden = cases.iter().any(|case| case.display.is_some());
    match default {
        None if !overridden => return Ok(None),
        None => require_all_annotated(cases)?,
        Some(DisplayArg::Fmt { fmt, args }) if !overridden => return Ok(Some(write(f, fmt, args))),
        Some(_) => {}
    }

    let arms = arms(cases, default, f)?;
    Ok(Some(quote! {
        match self {
            #(#arms)*
        }
    }))
}

fn require_all_annotated(cases: &[Case]) -> syn::Result<()> {
    let mut errors = Errors::default();
    for case in cases.iter().filter(|case| case.display.is_none()) {
        let ident = &case.variant.expect("enum cases are variants").ident;
        let msg = format!(
            "variant `{ident}` has no `#[display]`; annotate every variant \
             or set a default with `#[data(display(...))]`",
        );
        errors.push(syn::Error::new(ident.span(), msg));
    }
    errors.finish()
}

/// The variants that fall back to a type-level format share one arm, except `#[cfg]`-gated ones:
/// an or-pattern cannot carry attributes.
fn arms(
    cases: &[Case],
    default: Option<&DisplayArg>,
    f: &syn::Ident,
) -> syn::Result<Vec<TokenStream>> {
    let mut errors = Errors::default();
    let mut arms = Vec::new();
    let mut fmt_defaulted = Vec::new();
    for case in cases {
        match (&case.display, default) {
            (Some(own), _) => arms.extend(errors.ok(arm(case, own, f))),
            (None, Some(DisplayArg::Fmt { .. })) => fmt_defaulted.push(case),
            (None, Some(mode)) => arms.extend(errors.ok(arm(case, mode, f))),
            (None, None) => unreachable!("every variant has a display"),
        }
    }
    errors.finish()?;

    if let Some(DisplayArg::Fmt { fmt, args }) = default {
        let write = write(f, fmt, args);
        let (gated, shared): (Vec<_>, Vec<_>) = fmt_defaulted
            .into_iter()
            .partition(|case| !case.cfg.is_empty());
        for case in gated {
            let (cfg, pat) = (&case.cfg, case.wildcard());
            arms.push(quote! { #cfg #pat => #write, });
        }
        if !shared.is_empty() {
            let pats = shared.iter().map(|case| case.wildcard());
            arms.push(quote! { #(#pats)|* => #write, });
        }
    }
    Ok(arms)
}

/// A format binds the variant's fields; only the variant's own `#[display]` is a format here.
fn arm(case: &Case, display: &DisplayArg, f: &syn::Ident) -> syn::Result<TokenStream> {
    let var = case.variant.expect("enum cases are variants");
    let (pat, body) = match display {
        DisplayArg::Fmt { fmt, args } => (case.bind_all(), write(f, fmt, args)),
        DisplayArg::Name(_) => {
            let name = var.ident.unraw().to_string().to_snake_case();
            (case.wildcard(), quote! { #f.write_str(#name) })
        }
        DisplayArg::Doc(_) => {
            let text = doc_text(var)?;
            (case.wildcard(), quote! { #f.write_str(#text) })
        }
    };
    let cfg = &case.cfg;
    Ok(quote! { #cfg #pat => #body, })
}

fn write(f: &syn::Ident, fmt: &syn::LitStr, args: &[syn::Expr]) -> TokenStream {
    quote! { ::core::write!(#f, #fmt #(, #args)*) }
}

/// The variant's doc lines joined into one sentence, without the trailing period and with the
/// first letter lowercased unless the first word is an acronym or CamelCase.
fn doc_text(var: &syn::Variant) -> syn::Result<String> {
    let mut lines = Vec::new();
    for expr in docs::exprs(&var.attrs) {
        let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit),
            ..
        }) = expr
        else {
            let msg = "`display(doc)` needs literal doc comments";
            return Err(syn::Error::new_spanned(expr, msg));
        };
        let value = lit.value();
        lines.extend(
            value
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned),
        );
    }
    if lines.is_empty() {
        let msg = "`display(doc)` needs a doc comment on this variant";
        return Err(syn::Error::new(var.ident.span(), msg));
    }

    let text = lines.join(" ");
    Ok(lowercase_first(strip_period(&text)))
}

fn strip_period(text: &str) -> &str {
    if text.ends_with("..") {
        return text;
    }
    text.strip_suffix('.').unwrap_or(text)
}

fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let rest_of_word = chars.as_str().split(char::is_whitespace).next();
    if rest_of_word.is_some_and(|rest| rest.chars().any(char::is_uppercase)) {
        return text.to_owned();
    }
    first.to_lowercase().chain(chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(docs: &[&str]) -> String {
        let var: syn::Variant = syn::parse_quote! { #(#[doc = #docs])* V };
        doc_text(&var).unwrap()
    }

    #[test]
    fn doc_text_rules() {
        assert_eq!(text(&[" Invalid input."]), "invalid input");
        assert_eq!(text(&[" MessagePack map key."]), "MessagePack map key");
        assert_eq!(text(&[" HTTP failed."]), "HTTP failed");
        assert_eq!(text(&[" Invalid2 thing"]), "invalid2 thing");
        assert_eq!(text(&[" A thing."]), "a thing");
        assert_eq!(text(&[" Wait for it..."]), "wait for it...");
        assert_eq!(
            text(&[" First line.", "", "Second  ", " line."]),
            "first line. Second line"
        );
        assert_eq!(text(&["multi\n  line\n\n block"]), "multi line block");
    }
}
