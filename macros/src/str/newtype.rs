use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote, quote_spanned};
use syn::spanned::Spanned as _;

use crate::utils::{self, join, local, maybe};

use super::{KindImpls, Target, args::Args, construct, requires_struct};

pub fn expand<'a>(input: &'a syn::DeriveInput, args: &Args) -> syn::Result<KindImpls<'a>> {
    let fields = requires_struct(input, "newtype")?;
    let field = only_field(input, fields)?;
    let eva = &args.eva;
    let target = Target::new(input);

    let attrs = if args.traits.json_schema {
        json_schema_derive(field, eva)?
    } else {
        quote! {}
    };
    let mut impls = delegate(&target, fields, field);
    impls.extend(maybe(args.traits.pattern, || pattern(&target, field, eva)));

    Ok(KindImpls {
        target,
        attrs,
        impls,
    })
}

fn only_field<'a>(
    input: &syn::DeriveInput,
    fields: &'a syn::Fields,
) -> syn::Result<&'a syn::Field> {
    let mut iter = fields.iter();
    match (iter.next(), iter.next()) {
        (Some(field), None) => Ok(field),
        _ => {
            let msg = "`newtype` requires exactly one field";
            Err(match fields {
                syn::Fields::Unit => syn::Error::new_spanned(&input.ident, msg),
                _ => syn::Error::new_spanned(fields, msg),
            })
        }
    }
}

/// The schema of the inner type.
fn json_schema_derive(field: &syn::Field, eva: &syn::Path) -> syn::Result<TokenStream> {
    if utils::is_dollar_crate(eva) {
        let msg = "a `$crate` path cannot be passed to the schemars derive; \
                   opt out of it with `not(schemars)`";
        return Err(syn::Error::new_spanned(eva, msg));
    }
    let schemars = join(&join(eva, "_priv"), "schemars");
    let schemars_path = schemars.to_token_stream().to_string();
    // The derive gives a named field an object schema of its own.
    let transparent = maybe(
        field.ident.is_some(),
        || quote! { #[schemars(transparent)] },
    );

    Ok(quote! {
        #[derive(#schemars::JsonSchema)]
        #[schemars(crate = #schemars_path)]
        #transparent
    })
}

/// `as_str` and `FromStr` through the field.
fn delegate(target: &Target, fields: &syn::Fields, field: &syn::Field) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let inner = &field.ty;
    let inner_as_str = quote_spanned! {inner.span()=> <#inner>::as_str};
    let member = match &field.ident {
        Some(name) => quote! { #name },
        None => quote! { 0 },
    };
    let s = local("s");
    let parse = quote! { <#inner as ::core::str::FromStr>::from_str(#s)? };
    let construct = construct(fields, [parse].into_iter());

    quote! {
        impl #impl_generics #ty #where_clause {
            /// The string this value stores.
            #[inline]
            pub fn as_str(&self) -> &::core::primitive::str {
                #inner_as_str(&self.#member)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::core::str::FromStr for #ty #where_clause {
            type Err = <#inner as ::core::str::FromStr>::Err;

            #[inline]
            fn from_str(#s: &::core::primitive::str) -> ::core::result::Result<Self, Self::Err> {
                ::core::result::Result::Ok(Self #construct)
            }
        }
    }
}

fn pattern(target: &Target, field: &syn::Field, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let inner = &field.ty;
    let buf = local("buf");

    quote! {
        #[automatically_derived]
        impl #impl_generics #eva::str::HasPattern for #ty #where_clause {
            #[inline]
            fn pat_into(#buf: &mut ::std::string::String) {
                <#inner as #eva::str::HasPattern>::pat_into(#buf)
            }
        }
    }
}
