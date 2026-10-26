//! Validates the helper attributes (`#[display]`, `#[from]`, `#[source]`) and builds the
//! patterns that bind a case's fields.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::utils::Errors;

use super::{Item, Kind, display::DisplayArg};

/// A struct, or one variant of an enum: what one arm of `match self` sees.
pub struct Case<'a> {
    /// `Self` or `Self::Variant`.
    path: TokenStream,
    pub variant: Option<&'a syn::Variant>,
    /// The variant's `#[cfg]`s: rustc has not evaluated them yet, so everything generated for
    /// the variant repeats them.
    pub cfg: TokenStream,
    fields: &'a syn::Fields,
    /// `#[display(...)]`; only variants have one.
    pub display: Option<DisplayArg>,
    /// The `#[from]` field, which is the only field.
    pub from: Option<&'a syn::Field>,
    pub source: Option<(usize, &'a syn::Field)>,
}

impl Case<'_> {
    /// Binds named fields by name and tuple fields as `_0.._n`.
    pub fn bind_all(&self) -> TokenStream {
        let binds = self
            .fields
            .iter()
            .enumerate()
            .map(|(i, field)| field.ident.clone().unwrap_or_else(|| format_ident!("_{i}")));
        self.with_path(delimit(self.fields, quote! { #(#binds),* }))
    }

    /// Binds field `index` to `bind` and ignores the rest.
    pub fn bind_one(&self, index: usize, bind: &syn::Ident) -> TokenStream {
        let pat = match self.fields.iter().nth(index).and_then(|f| f.ident.as_ref()) {
            Some(name) => quote! { { #name: #bind, .. } },
            None => {
                let skipped = (0..index).map(|_| quote! { _ });
                quote! { (#(#skipped,)* #bind, ..) }
            }
        };
        self.with_path(pat)
    }

    pub fn wildcard(&self) -> TokenStream {
        self.with_path(delimit(self.fields, quote! { .. }))
    }

    /// Builds the case from `value`, its only field.
    pub fn construct(&self, value: &syn::Ident) -> TokenStream {
        let value = match self.fields.iter().next().and_then(|f| f.ident.as_ref()) {
            Some(name) => quote! { #name: #value },
            None => quote! { #value },
        };
        self.with_path(delimit(self.fields, value))
    }

    fn with_path(&self, rest: TokenStream) -> TokenStream {
        let path = &self.path;
        quote! { #path #rest }
    }
}

fn delimit(fields: &syn::Fields, inside: TokenStream) -> TokenStream {
    match fields {
        syn::Fields::Named(_) => quote! { { #inside } },
        syn::Fields::Unnamed(_) => quote! { (#inside) },
        syn::Fields::Unit => quote! {},
    }
}

/// The cases of `item`, with their helper attributes validated.
pub fn collect(item: &Item, error: bool) -> syn::Result<Vec<Case<'_>>> {
    let mut errors = Errors::default();
    reject(&item.attrs, &["display", "from", "source"], &mut errors);
    let cases = match &item.kind {
        Kind::Struct(fields) => vec![case(None, fields, error, &mut errors)],
        Kind::Enum(variants) => variants
            .iter()
            .map(|var| {
                reject(&var.attrs, &["from", "source"], &mut errors);
                case(Some(var), &var.fields, error, &mut errors)
            })
            .collect(),
    };

    errors.finish()?;
    Ok(cases)
}

fn case<'a>(
    variant: Option<&'a syn::Variant>,
    fields: &'a syn::Fields,
    error: bool,
    errors: &mut Errors,
) -> Case<'a> {
    let (path, cfg, owner) = match variant {
        Some(var) => {
            let ident = &var.ident;
            let cfg = var.attrs.iter().filter(|a| a.path().is_ident("cfg"));
            (quote! { Self::#ident }, quote! { #(#cfg)* }, "variant")
        }
        None => (quote! { Self }, quote! {}, "struct"),
    };
    let mut case = Case {
        path,
        variant,
        cfg,
        fields,
        display: variant.and_then(|var| variant_display(var, errors)),
        from: None,
        source: None,
    };
    for (index, field) in fields.iter().enumerate() {
        reject(&field.attrs, &["display"], errors);
        for attr in &field.attrs {
            let checked = if attr.path().is_ident("from") {
                mark_from(&mut case, field, attr, owner)
            } else if attr.path().is_ident("source") {
                mark_source(&mut case, index, field, attr, error, owner)
            } else {
                Ok(())
            };
            errors.ok(checked);
        }
    }
    case
}

fn mark_from<'a>(
    case: &mut Case<'a>,
    field: &'a syn::Field,
    attr: &syn::Attribute,
    owner: &str,
) -> syn::Result<()> {
    no_args(attr)?;
    if case.fields.len() != 1 {
        let msg = format!("`#[from]` needs the {owner} to have exactly one field");
        return Err(syn::Error::new_spanned(attr, msg));
    }
    if case.from.replace(field).is_some() {
        return Err(syn::Error::new_spanned(attr, "duplicate `#[from]`"));
    }
    Ok(())
}

fn mark_source<'a>(
    case: &mut Case<'a>,
    index: usize,
    field: &'a syn::Field,
    attr: &syn::Attribute,
    error: bool,
    owner: &str,
) -> syn::Result<()> {
    no_args(attr)?;
    if !error {
        let msg = "`#[source]` requires `#[data(error)]`";
        return Err(syn::Error::new_spanned(attr, msg));
    }
    if case.source.replace((index, field)).is_some() {
        let msg = format!("duplicate `#[source]`; a {owner} has at most one source");
        return Err(syn::Error::new_spanned(attr, msg));
    }
    Ok(())
}

fn no_args(attr: &syn::Attribute) -> syn::Result<()> {
    match &attr.meta {
        syn::Meta::Path(_) => Ok(()),
        meta => {
            let name = meta
                .path()
                .get_ident()
                .expect("helper attributes are idents");
            let msg = format!("`#[{name}]` takes no arguments");
            Err(syn::Error::new_spanned(meta, msg))
        }
    }
}

/// Parses the variant's `#[display(...)]`; later ones are duplicates.
fn variant_display(var: &syn::Variant, errors: &mut Errors) -> Option<DisplayArg> {
    let mut attrs = var.attrs.iter().filter(|a| a.path().is_ident("display"));
    let first = attrs.next()?;
    for dup in attrs {
        errors.push(syn::Error::new_spanned(dup, "duplicate `#[display]`"));
    }
    errors.ok(first.parse_args())
}

/// Reports the `misplaced` helper attributes found in `attrs`.
fn reject(attrs: &[syn::Attribute], misplaced: &[&str], errors: &mut Errors) {
    for attr in attrs {
        let Some(name) = misplaced.iter().find(|name| attr.path().is_ident(name)) else {
            continue;
        };
        let msg = match *name {
            "display" => {
                "`#[display]` belongs on enum variants; \
                 use `#[data(display(...))]` for the whole type"
            }
            "from" => "`#[from]` belongs on the only field of a struct or variant",
            _ => "`#[source]` belongs on a field",
        };
        errors.push(syn::Error::new_spanned(attr, msg));
    }
}
