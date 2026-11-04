use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote, quote_spanned};
use syn::{
    Token,
    parse::{Parse, ParseStream},
    spanned::Spanned as _,
};

use crate::{
    opts::{self, Opts, Table},
    utils,
};

use self::{attrs::Routed, validate::Params};

mod attrs;
pub(crate) mod scope;
mod trace;
mod validate;

const OPTIONS: Table = Table {
    keys: &[opts::CRATE],
    not: &[&["trace"]],
};

struct Args {
    eva: syn::Path,
    method: Option<syn::Path>,
    trace: bool,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method =
            if !input.is_empty() && !input.peek2(Token![=]) && !input.peek2(syn::token::Paren) {
                Some(input.call(syn::Path::parse_mod_style)?)
            } else {
                None
            };
        let opts = if method.is_some() {
            Opts::parse_after_positional(input, &OPTIONS)?
        } else {
            Opts::parse(input, &OPTIONS)?
        };
        Ok(Self {
            eva: opts.krate().unwrap_or_else(crate::runtime),
            method,
            trace: opts.not("trace").is_none(),
        })
    }
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let item = syn::parse2::<syn::ItemFn>(item).and_then(|item| {
        let params = validate::item(&item)?;
        Ok((params, item))
    });
    let (args, (params, item)) = utils::both(syn::parse2::<Args>(args), item)?;
    let attrs = attrs::route(&item.attrs);

    let mut out = unit_struct(&item, &attrs);
    out.extend(endpoint_impl(&args, &item, &params, &attrs));
    Ok(out)
}

fn unit_struct(item: &syn::ItemFn, attrs: &Routed) -> TokenStream {
    let (vis, ident) = (&item.vis, &item.sig.ident);
    let attrs = &attrs.on_struct;
    quote! {
        #(#attrs)*
        #[derive(
            ::core::clone::Clone,
            ::core::marker::Copy,
            ::core::fmt::Debug,
            ::core::default::Default,
        )]
        #vis struct #ident;
    }
}

// No `#[automatically_derived]`: it would silence `unused_variables` in the user's body.
fn endpoint_impl(
    options: &Args,
    item: &syn::ItemFn,
    Params { state, args }: &Params,
    attrs: &Routed,
) -> TokenStream {
    let eva = &options.eva;
    let syn::ItemFn { sig, block, .. } = item;
    let ident = &sig.ident;
    let (impl_generics, _, where_clause) = sig.generics.split_for_impl();
    let (state_ty, args_ty) = (&state.ty, &args.ty);

    let (output, output_span) = match &sig.output {
        syn::ReturnType::Default => (quote! { () }, ident.span()),
        syn::ReturnType::Type(_, ty) => (ty.to_token_stream(), ty.span()),
    };
    // Send errors of the body's future point at this token.
    let returns = quote_spanned! {output_span=> Self::Output };
    let this = quote_spanned! {Span::mixed_site()=> self };

    let alias = utils::local("Endpoint");
    let self_ty = if attrs.deprecated { &alias } else { ident };
    let (impl_attrs, call_attrs) = (&attrs.on_impl, &attrs.on_call);
    let trace = trace::attribute(options, item);
    let imp = quote! {
        #(#impl_attrs)*
        impl #impl_generics #eva::_priv::Endpoint<#args_ty, #state_ty>
            for #self_ty #where_clause
        {
            type Output = #output;

            #trace
            #(#call_attrs)*
            async fn call(&#this, #state, #args) -> #returns #block
        }
    };
    if !attrs.deprecated && options.method.is_none() {
        return imp;
    }
    // Keep the impl in the caller's module so its `self`/`super` paths retain their meaning.
    let method = options.method.as_ref().map(scope::declare);
    // Only the alias names the deprecated struct, so the body keeps its deprecation lints.
    let deprecated = attrs.deprecated.then(|| {
        quote! {
            #[allow(deprecated)]
            type #alias = #ident;
        }
    });
    quote! {
        const _: () = {
            #method
            #deprecated
            #imp
        };
    }
}

/// The endpoint's struct without the impl, so its name stays resolvable after an error, unless the
/// fn has a receiver: it then sits in an `impl`, where a struct is another error.
pub fn fallback(item: TokenStream) -> TokenStream {
    match syn::parse2::<syn::ItemFn>(item) {
        Ok(f) if f.sig.receiver().is_none() => unit_struct(&f, &attrs::route(&f.attrs)),
        _ => quote! {},
    }
}
