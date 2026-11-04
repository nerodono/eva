use crate::utils::maybe;
pub use eva_macros_common::data_options::Args;
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

pub fn derives(
    d: &eva_macros_common::data_options::Derives,
    eva: &syn::Path,
    item_attrs: &[syn::Attribute],
) -> TokenStream {
    let std_derives = [
        (d.common.debug, quote! { ::core::fmt::Debug }),
        (d.clone, quote! { ::core::clone::Clone }),
        (d.copy, quote! { ::core::marker::Copy }),
        (d.partial_eq, quote! { ::core::cmp::PartialEq }),
        (d.eq, quote! { ::core::cmp::Eq }),
        (d.partial_ord, quote! { ::core::cmp::PartialOrd }),
        (d.ord, quote! { ::core::cmp::Ord }),
    ];
    let std_derives = std_derives
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, path)| quote! { #[derive(#path)] });

    let serde = quote! { #eva::_priv::serde };
    let serde_attrs = maybe(d.common.serialize || d.common.deserialize, || {
        let krate = serde.to_string();
        let ser = maybe(
            d.common.serialize,
            || quote! { #[derive(#serde::Serialize)] },
        );
        let de = maybe(
            d.common.deserialize,
            || quote! { #[derive(#serde::Deserialize)] },
        );
        let rename = maybe(!has_rename_all(item_attrs), || {
            quote! { #[serde(rename_all = "snake_case")] }
        });
        quote! {
            #ser
            #de
            #[serde(crate = #krate)]
            #rename
        }
    });

    let json_schema = maybe(d.common.json_schema, || {
        let schemars = quote! { #eva::_priv::schemars };
        let krate = schemars.to_string();
        quote! {
            #[derive(#schemars::JsonSchema)]
            #[schemars(crate = #krate)]
        }
    });

    quote! {
        #(#std_derives)*
        #serde_attrs
        #json_schema
    }
}

/// Whether a container `#[serde(...)]` already sets `rename_all`, in any form.
fn has_rename_all(attrs: &[syn::Attribute]) -> bool {
    let sets_rename_all = |list: &syn::MetaList| {
        let mut entry_start = true;
        list.tokens.clone().into_iter().any(|tt| {
            let found = entry_start && matches!(&tt, TokenTree::Ident(i) if i == "rename_all");
            entry_start = matches!(&tt, TokenTree::Punct(p) if p.as_char() == ',');
            found
        })
    };
    attrs.iter().any(|attr| match &attr.meta {
        syn::Meta::List(list) => list.path.is_ident("serde") && sets_rename_all(list),
        _ => false,
    })
}
