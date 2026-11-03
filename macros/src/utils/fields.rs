use proc_macro2::TokenStream;
use quote::quote;

pub fn requires_struct<'a>(
    input: &'a syn::DeriveInput,
    kind: &str,
) -> syn::Result<&'a syn::DataStruct> {
    let keyword = match &input.data {
        syn::Data::Struct(data) => return Ok(data),
        syn::Data::Enum(data) => data.enum_token.span,
        syn::Data::Union(data) => data.union_token.span,
    };
    Err(syn::Error::new(
        keyword,
        format!("`{kind}` requires a struct"),
    ))
}

pub fn only_field<'a>(ident: &syn::Ident, fields: &'a syn::Fields) -> syn::Result<&'a syn::Field> {
    let mut iter = fields.iter();
    match (iter.next(), iter.next()) {
        (Some(field), None) => Ok(field),
        _ => {
            let msg = "`newtype` requires exactly one field";
            Err(match fields {
                syn::Fields::Unit => syn::Error::new_spanned(ident, msg),
                _ => syn::Error::new_spanned(fields, msg),
            })
        }
    }
}

/// `{ a: v1, b: v2 }`, `(v1, v2)` or nothing, matching the shape of `fields`.
pub fn construct(fields: &syn::Fields, values: impl Iterator<Item = TokenStream>) -> TokenStream {
    let values = fields
        .iter()
        .zip(values)
        .map(|(field, value)| match &field.ident {
            Some(name) => quote! { #name: #value },
            None => value,
        });
    delimit(fields, quote!(#(#values),*))
}

/// Braces, parentheses or nothing, for field constructors and patterns alike.
pub fn delimit(fields: &syn::Fields, inside: TokenStream) -> TokenStream {
    match fields {
        syn::Fields::Named(_) => quote! { { #inside } },
        syn::Fields::Unnamed(_) => quote! { (#inside) },
        syn::Fields::Unit => quote! {},
    }
}
