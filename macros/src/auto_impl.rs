//! Derived from auto_impl 1.3.0 (https://github.com/auto-impl-rs/auto_impl), MIT, see
//! `LICENSE-auto_impl`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    FnArg, Pat, ReturnType, Token, TraitItem, Type, TypeParamBound, WherePredicate,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
};

use crate::utils::{Errors, local};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Proxy {
    Ref,
    RefMut,
    Box,
    Rc,
    Arc,
}

impl Parse for Proxy {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.parse::<Option<Token![&]>>()?.is_some() {
            return Ok(match input.parse::<Option<Token![mut]>>()? {
                Some(_) => Self::RefMut,
                None => Self::Ref,
            });
        }
        let ident: syn::Ident = input.parse()?;
        match ident.to_string().as_str() {
            "Box" => Ok(Self::Box),
            "Rc" => Ok(Self::Rc),
            "Arc" => Ok(Self::Arc),
            _ => Err(syn::Error::new(
                ident.span(),
                "expected `&`, `&mut`, `Box`, `Rc` or `Arc`",
            )),
        }
    }
}

impl Proxy {
    fn name(self) -> &'static str {
        match self {
            Self::Ref => "&",
            Self::RefMut => "&mut",
            Self::Box => "Box",
            Self::Rc => "Rc",
            Self::Arc => "Arc",
        }
    }

    fn allows(self, receiver: Receiver) -> bool {
        match receiver {
            Receiver::None | Receiver::Ref => true,
            Receiver::Mut => matches!(self, Self::RefMut | Self::Box),
            Receiver::Value => self == Self::Box,
        }
    }
}

#[derive(Clone, Copy)]
enum Receiver {
    None,
    Ref,
    Mut,
    Value,
}

impl Receiver {
    fn of(sig: &syn::Signature) -> Self {
        match sig.inputs.first() {
            Some(FnArg::Receiver(r)) if r.reference.is_none() => Self::Value,
            Some(FnArg::Receiver(r)) if r.mutability.is_none() => Self::Ref,
            Some(FnArg::Receiver(_)) => Self::Mut,
            _ => Self::None,
        }
    }
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let proxies =
        syn::parse::Parser::parse2(Punctuated::<Proxy, Token![,]>::parse_terminated, args)?;
    let item = syn::parse2::<syn::ItemTrait>(item)?;
    let methods = item.items.iter().filter_map(|item| match item {
        TraitItem::Fn(m) => Some(m),
        _ => None,
    });

    let mut errors = Errors::default();
    let mut sized = false;
    let mut extra_bounds = Vec::new();
    for m in methods.clone() {
        sized |= matches!(Receiver::of(&m.sig), Receiver::Value)
            || matches!(&m.sig.output, ReturnType::Type(_, ty) if is_self(ty));
        for pred in self_predicates(&m.sig) {
            for bound in &pred.bounds {
                match bound {
                    TypeParamBound::Trait(b)
                        if matches!(b.modifier, syn::TraitBoundModifier::None)
                            && b.path.is_ident("Sized") =>
                    {
                        sized = true
                    }
                    // Method-level `Self: Trait` bounds can only hold in the impl if `T` has them.
                    TypeParamBound::Trait(b) => extra_bounds.push(b),
                    _ => {}
                }
            }
        }
    }
    for item in &item.items {
        if !matches!(
            item,
            TraitItem::Fn(_) | TraitItem::Const(_) | TraitItem::Type(_)
        ) {
            errors.push(syn::Error::new(item.span(), "unsupported trait item"));
        }
    }
    for proxy in &proxies {
        for m in methods.clone() {
            if !proxy.allows(Receiver::of(&m.sig)) {
                errors.push(syn::Error::new(
                    m.sig.inputs.span(),
                    format!(
                        "this receiver cannot be forwarded through `{}`",
                        proxy.name()
                    ),
                ));
            }
        }
    }

    let ty = local("T");
    let lt = syn::Lifetime::new("'__auto_impl", Span::call_site());
    let relax = (!sized).then(|| quote! { + ?::core::marker::Sized });
    let trait_ident = &item.ident;
    let unsafety = &item.unsafety;
    let supertraits = &item.supertraits;
    let (_, trait_generics, _) = item.generics.split_for_impl();
    let trait_path = quote! { #trait_ident #trait_generics };
    let items = item
        .items
        .iter()
        .filter_map(|item| errors.ok(forward(item, &ty)))
        .collect::<Vec<_>>();
    errors.finish()?;

    let impls = proxies.iter().map(|&proxy| {
        let mut generics = item.generics.clone();
        let self_ty = match proxy {
            Proxy::Ref => quote! { &#lt #ty },
            Proxy::RefMut => quote! { &#lt mut #ty },
            Proxy::Box => quote! { alloc::boxed::Box<#ty> },
            Proxy::Rc => quote! { alloc::rc::Rc<#ty> },
            Proxy::Arc => quote! { alloc::sync::Arc<#ty> },
        };
        let is_ref = matches!(proxy, Proxy::Ref | Proxy::RefMut);
        if is_ref {
            generics.params.insert(0, syn::parse_quote!(#lt));
        }
        generics
            .params
            .push(syn::parse_quote!(#ty: #trait_path #relax #(+ #extra_bounds)*));
        if !supertraits.is_empty() {
            generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote!(#self_ty: #supertraits));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let alloc = (!is_ref).then(|| quote! { extern crate alloc; });
        quote! {
            const _: () = {
                #alloc
                #unsafety impl #impl_generics #trait_path for #self_ty #where_clause {
                    #(#items)*
                }
            };
        }
    });

    Ok(quote! { #item #(#impls)* })
}

fn forward(item: &TraitItem, ty: &syn::Ident) -> syn::Result<TokenStream> {
    Ok(match item {
        TraitItem::Const(c) => {
            let (attrs, name, cty) = (cfgs(&c.attrs), &c.ident, &c.ty);
            quote! { #(#attrs)* const #name: #cty = #ty::#name; }
        }
        TraitItem::Type(t) => {
            let (attrs, name) = (cfgs(&t.attrs), &t.ident);
            let (impl_generics, type_generics, where_clause) = t.generics.split_for_impl();
            quote! { #(#attrs)* type #name #impl_generics = #ty::#name #type_generics #where_clause; }
        }
        TraitItem::Fn(m) => {
            let attrs = cfgs(&m.attrs);
            let mut sig = m.sig.clone();
            let mut errors = Errors::default();
            let mut args = Vec::new();
            for input in &mut sig.inputs {
                let FnArg::Typed(arg) = input else { continue };
                match &mut *arg.pat {
                    Pat::Ident(pat) if pat.subpat.is_none() => {
                        pat.by_ref = None;
                        pat.mutability = None;
                        args.push(pat.ident.clone());
                    }
                    pat => errors.push(syn::Error::new(
                        pat.span(),
                        "expected a plain argument name",
                    )),
                }
            }
            errors.finish()?;

            // Lifetimes are left out: late-bound ones cannot be named explicitly.
            let turbofish = sig
                .generics
                .params
                .iter()
                .filter_map(|p| match p {
                    syn::GenericParam::Type(t) => Some(&t.ident),
                    syn::GenericParam::Const(c) => Some(&c.ident),
                    syn::GenericParam::Lifetime(_) => None,
                })
                .collect::<Vec<_>>();
            let turbofish = (!turbofish.is_empty()).then(|| quote! { ::<#(#turbofish),*> });
            let name = &sig.ident;
            let receiver = match Receiver::of(&sig) {
                Receiver::None => None,
                Receiver::Value => Some(quote! { *self, }),
                Receiver::Ref | Receiver::Mut => Some(quote! { self, }),
            };
            let await_ = sig.asyncness.map(|_| quote! { .await });
            quote! { #(#attrs)* #sig { #ty::#name #turbofish(#receiver #(#args),*) #await_ } }
        }
        _ => unreachable!("rejected by expand"),
    })
}

fn is_self(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.qself.is_none() && p.path.is_ident("Self"))
}

fn self_predicates(sig: &syn::Signature) -> impl Iterator<Item = &syn::PredicateType> {
    sig.generics
        .where_clause
        .iter()
        .flat_map(|w| &w.predicates)
        .filter_map(|pred| match pred {
            WherePredicate::Type(p) if is_self(&p.bounded_ty) => Some(p),
            _ => None,
        })
}

fn cfgs(attrs: &[syn::Attribute]) -> impl Iterator<Item = &syn::Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("cfg"))
}
