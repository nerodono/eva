use std::{
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
};

use proc_macro2::{Group, TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
    visit_mut::VisitMut,
};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

pub(super) fn unique(prefix: &str, name: &syn::Ident) -> syn::Ident {
    format_ident!(
        "__eva_{}_{}_{}",
        prefix,
        name.to_string().trim_start_matches("r#"),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Default)]
struct Options {
    export: Option<Option<Vec<syn::Ident>>>,
    result: Option<syn::Path>,
    error: Option<syn::Type>,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut opts = Self::default();
        while !input.is_empty() {
            let name: syn::Ident = input.parse()?;
            match name.to_string().as_str() {
                "export" if opts.export.is_none() => {
                    opts.export = Some(if input.peek(syn::token::Paren) {
                        let content;
                        syn::parenthesized!(content in input);
                        Some(
                            Punctuated::<syn::Ident, Token![,]>::parse_terminated(&content)?
                                .into_iter()
                                .collect(),
                        )
                    } else {
                        None
                    });
                }
                "result" if opts.result.is_none() => {
                    input.parse::<Token![=]>()?;
                    opts.result = Some(input.parse()?);
                }
                "error" if opts.error.is_none() => {
                    input.parse::<Token![=]>()?;
                    opts.error = Some(input.parse()?);
                }
                _ => {
                    return Err(syn::Error::new(
                        name.span(),
                        "unknown or duplicate service option",
                    ));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(opts)
    }
}

fn qualify_this(tokens: TokenStream, root: &TokenStream) -> syn::Result<TokenStream> {
    let tokens: Vec<_> = tokens.into_iter().collect();
    let mut output = TokenStream::new();
    let mut i = 0;
    while i < tokens.len() {
        if let [
            TokenTree::Ident(name),
            TokenTree::Punct(bang),
            TokenTree::Group(body),
            ..,
        ] = &tokens[i..]
            && name == "this"
            && bang.as_char() == '!'
        {
            if body.stream().is_empty() {
                return Err(syn::Error::new(
                    body.span(),
                    "expected tokens in this![...]",
                ));
            }
            let tail = qualify_this(body.stream(), root)?;
            output.extend(quote!(#root::#tail));
            i += 3;
        } else {
            let token = match &tokens[i] {
                TokenTree::Group(group) => {
                    let mut expanded =
                        Group::new(group.delimiter(), qualify_this(group.stream(), root)?);
                    expanded.set_span(group.span());
                    TokenTree::Group(expanded)
                }
                token => token.clone(),
            };
            output.extend([token]);
            i += 1;
        }
    }
    Ok(output)
}

struct ErrorVariant {
    ty: syn::Path,
    name: syn::Ident,
}
impl Parse for ErrorVariant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::Ident) && input.peek2(syn::token::Paren) {
            let name = input.parse()?;
            let content;
            syn::parenthesized!(content in input);
            let ty = content.parse()?;
            if !content.is_empty() {
                return Err(content.error("expected exactly one error type in Variant(Type)"));
            }
            return Ok(Self { ty, name });
        }
        let ty: syn::Path = input.parse()?;
        let name = ty.segments.last().unwrap().ident.clone();
        Ok(Self { ty, name })
    }
}

fn data_attrs(attrs: Vec<syn::Attribute>, error: bool) -> TokenStream {
    let has_data = attrs.iter().any(|attr| attr.path().is_ident("data"));
    let default = (!has_data).then(|| {
        if error {
            quote!(#[::eva::data(error)])
        } else {
            quote!(#[::eva::data])
        }
    });
    let attrs = attrs.into_iter().map(|mut attr| {
        if attr.path().is_ident("data") {
            match &mut attr.meta {
                syn::Meta::Path(p) => *p = syn::parse_quote!(::eva::data),
                syn::Meta::List(l) => l.path = syn::parse_quote!(::eva::data),
                _ => {}
            }
        }
        attr
    });
    quote!(#default #(#attrs)*)
}

fn attributes(attr: &syn::Attribute) -> syn::Result<Vec<syn::Attribute>> {
    let metas = attr.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;
    Ok(metas
        .into_iter()
        .map(|meta| syn::parse_quote!(#[#meta]))
        .collect())
}

struct Lifetimes {
    names: Vec<syn::Lifetime>,
    infer: bool,
}
impl VisitMut for Lifetimes {
    fn visit_type_reference_mut(&mut self, node: &mut syn::TypeReference) {
        if self.infer && node.lifetime.is_none() {
            node.lifetime = Some(syn::parse_quote!('__a));
        }
        syn::visit_mut::visit_type_reference_mut(self, node);
    }
    fn visit_lifetime_mut(&mut self, node: &mut syn::Lifetime) {
        if self.infer && node.ident == "_" {
            *node = syn::parse_quote!('__a);
        }
        if node.ident != "static" && !self.names.iter().any(|lt| lt.ident == node.ident) {
            self.names.push(node.clone());
        }
    }
}

fn generics(names: &[syn::Lifetime]) -> TokenStream {
    if names.is_empty() {
        quote!()
    } else {
        quote!(<#(#names),*>)
    }
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let opts = syn::parse2::<Options>(args)?;
    let item = syn::parse2::<syn::ItemTrait>(item)?;
    if !item.generics.params.is_empty() || !item.supertraits.is_empty() || item.unsafety.is_some() {
        return Err(syn::Error::new(
            item.span(),
            "service traits do not take generics, supertraits or unsafe",
        ));
    }
    let name = &item.ident;
    let vis = &item.vis;
    let docs = &item.attrs;
    let result = opts
        .result
        .unwrap_or_else(|| syn::parse_quote!(::core::result::Result));
    let mut modules = Vec::new();
    let mut methods = Vec::new();
    let mut clients = Vec::new();
    let mut entries = Vec::new();
    let mut names = HashSet::new();
    let mut method_names = Vec::new();
    let mut declarations: Vec<(&syn::TraitItemFn, Vec<syn::Item>)> = Vec::new();
    for entry in &item.items {
        if let syn::TraitItem::Fn(method) = entry {
            declarations.push((method, Vec::new()));
            continue;
        }
        if let syn::TraitItem::Macro(block) = entry
            && block.mac.path.is_ident("embed")
        {
            if !block.attrs.is_empty() {
                return Err(syn::Error::new(
                    block.span(),
                    "put attributes on items inside embed!",
                ));
            }
            let Some((_, embedded)) = declarations.last_mut() else {
                return Err(syn::Error::new(
                    block.span(),
                    "embed! must follow a service method",
                ));
            };
            let tokens = qualify_this(block.mac.tokens.clone(), &quote!(self))?;
            embedded.extend(syn::parse2::<syn::File>(tokens)?.items);
            continue;
        }
        return Err(syn::Error::new(
            entry.span(),
            "expected a service method or embed! { ... }",
        ));
    }
    for (original, embedded) in declarations {
        let method = syn::parse2::<syn::TraitItemFn>(qualify_this(
            original.to_token_stream(),
            &quote!(self),
        )?)?;
        let sig = &method.sig;
        let ident = &sig.ident;
        if ["new", "into_inner", "inner_mut"].contains(&ident.to_string().as_str()) {
            return Err(syn::Error::new(
                ident.span(),
                "method name is reserved by Client",
            ));
        }
        if !names.insert(ident.to_string()) {
            return Err(syn::Error::new(ident.span(), "duplicate method"));
        }
        method_names.push(ident.clone());
        if method.default.is_some()
            || sig.asyncness.is_some()
            || sig.constness.is_some()
            || sig.unsafety.is_some()
            || sig.abi.is_some()
            || sig.variadic.is_some()
            || sig.generics.where_clause.is_some()
            || sig
                .generics
                .params
                .iter()
                .any(|p| !matches!(p, syn::GenericParam::Lifetime(_)))
        {
            return Err(syn::Error::new(
                sig.span(),
                "expected `fn method(arguments) -> Success;` with optional lifetime parameters",
            ));
        }
        let mut flags = Vec::<syn::Ident>::new();
        let mut errors = None;
        let mut error_ty = opts
            .error
            .as_ref()
            .map(|ty| syn::parse2::<syn::Type>(qualify_this(ty.to_token_stream(), &quote!(self))?))
            .transpose()?;
        let mut args_attrs = Vec::new();
        let mut ok_attrs = Vec::new();
        let mut err_attrs = Vec::new();
        let mut method_attrs = Vec::new();
        for attr in &method.attrs {
            if attr.path().is_ident("flags") {
                flags.extend(
                    attr.parse_args_with(Punctuated::<syn::Ident, Token![,]>::parse_terminated)?,
                );
            } else if attr.path().is_ident("errors") {
                if errors.is_some() {
                    return Err(syn::Error::new(attr.span(), "duplicate errors attribute"));
                }
                errors = Some(
                    attr.parse_args_with(Punctuated::<ErrorVariant, Token![,]>::parse_terminated)?,
                );
            } else if attr.path().is_ident("error") {
                error_ty = Some(attr.parse_args::<syn::Type>()?);
            } else if attr.path().is_ident("args") {
                args_attrs.extend(attributes(attr)?);
            } else if attr.path().is_ident("ok") {
                ok_attrs.extend(attributes(attr)?);
            } else if attr.path().is_ident("err") {
                err_attrs.extend(attributes(attr)?);
            } else if attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr") {
                return Err(syn::Error::new(
                    attr.span(),
                    "conditional service methods are not supported by endpoint metadata",
                ));
            } else {
                method_attrs.push(attr.clone());
            }
        }
        if errors.is_some() && error_ty.is_some() {
            return Err(syn::Error::new(
                sig.span(),
                "use either errors(...) or a shared error type",
            ));
        }
        let doc_strings = method_attrs
            .iter()
            .filter_map(|a| {
                if let syn::Meta::NameValue(v) = &a.meta
                    && a.path().is_ident("doc")
                {
                    return Some(v.value.clone());
                }
                None
            })
            .collect::<Vec<_>>();
        let mut lifetimes = Lifetimes {
            names: Vec::new(),
            infer: true,
        };
        let mut fields = Vec::new();
        let mut params = Vec::new();
        let mut values = Vec::new();
        let mut field_names = HashSet::new();
        for (arg, original_arg) in sig.inputs.iter().zip(&original.sig.inputs) {
            let syn::FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new(
                    arg.span(),
                    "service methods do not declare self",
                ));
            };
            let syn::Pat::Ident(pat) = &*arg.pat else {
                return Err(syn::Error::new(arg.pat.span(), "expected a named argument"));
            };
            if pat.by_ref.is_some() || pat.subpat.is_some() {
                return Err(syn::Error::new(pat.span(), "expected a named argument"));
            }
            if !field_names.insert(pat.ident.to_string()) {
                return Err(syn::Error::new(pat.span(), "duplicate argument"));
            }
            let field = &pat.ident;
            let mut ty = (*arg.ty).clone();
            lifetimes.visit_type_mut(&mut ty);
            let syn::FnArg::Typed(original_arg) = original_arg else {
                unreachable!()
            };
            let mut param_ty: syn::Type = syn::parse2(qualify_this(
                original_arg.ty.to_token_stream(),
                &quote!(self::#ident),
            )?)?;
            lifetimes.visit_type_mut(&mut param_ty);
            let attrs = &arg.attrs;
            fields.push(quote!(#(#attrs)* pub #field: #ty));
            params.push(quote!(#field: #param_ty));
            values.push(field);
        }
        let args_generics = generics(&lifetimes.names);
        let mut ok_lifetimes = Lifetimes {
            names: Vec::new(),
            infer: false,
        };
        let ok_tokens = match &sig.output {
            syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Macro(m) if m.mac.path.is_ident("ok") && matches!(m.mac.delimiter, syn::MacroDelimiter::Brace(_))) =>
            {
                let syn::Type::Macro(m) = &**ty else {
                    unreachable!()
                };
                let parser = |input: ParseStream| {
                    let attrs = syn::Attribute::parse_inner(input)?;
                    let fields = Punctuated::<syn::Field, Token![,]>::parse_terminated_with(
                        input,
                        syn::Field::parse_named,
                    )?;
                    Ok((attrs, fields))
                };
                let (attrs, mut fields) = parser.parse2(m.mac.tokens.clone())?;
                ok_attrs.extend(attrs.into_iter().map(|mut attr| {
                    attr.style = syn::AttrStyle::Outer;
                    attr
                }));
                // Parse fields without requiring callers to spell `pub`.
                for field in &mut fields {
                    field.vis = syn::parse_quote!(pub);
                    ok_lifetimes.visit_type_mut(&mut field.ty);
                }
                let ok_generics = generics(&ok_lifetimes.names);
                let attrs = data_attrs(ok_attrs, false);
                quote!(#attrs pub struct Ok #ok_generics { #fields })
            }
            ret => {
                let mut ty: syn::Type = match ret {
                    syn::ReturnType::Type(_, ty) => (**ty).clone(),
                    _ => syn::parse_quote!(()),
                };
                if let syn::Type::Macro(mac) = &ty
                    && mac.mac.path.is_ident("ok")
                {
                    let parser = |input: ParseStream| {
                        let attrs = syn::Attribute::parse_outer(input)?;
                        let ty = input.parse::<syn::Type>()?;
                        Ok((attrs, ty))
                    };
                    let (attrs, inner) = parser.parse2(mac.mac.tokens.clone())?;
                    ok_attrs.extend(attrs);
                    ty = inner;
                }
                ok_lifetimes.visit_type_mut(&mut ty);
                let ok_generics = generics(&ok_lifetimes.names);
                let no_serde = ok_attrs.iter().any(|a| {
                    a.path().is_ident("data") && a.to_token_stream().to_string().contains("serde")
                });
                let transparent = (!no_serde
                    && !ok_attrs.iter().any(|a| a.path().is_ident("serde")))
                .then(|| quote!(#[serde(transparent)]));
                let attrs = data_attrs(ok_attrs, false);
                quote!(#attrs #transparent pub struct Ok #ok_generics (#[from] pub #ty);)
            }
        };
        for lt in &ok_lifetimes.names {
            if !lifetimes.names.iter().any(|l| l.ident == lt.ident) {
                return Err(syn::Error::new(
                    lt.span(),
                    "response lifetime must occur in an argument",
                ));
            }
        }
        let ok_generics = generics(&ok_lifetimes.names);
        let err_tokens = if let Some(ty) = error_ty {
            quote!(pub type Err = #ty;)
        } else {
            let errors = errors.unwrap_or_default();
            let mut variants = HashSet::new();
            let mut tokens = Vec::new();
            for error in errors {
                let ErrorVariant { name, ty } = error;
                if !variants.insert(name.to_string()) {
                    return Err(syn::Error::new(
                        name.span(),
                        "duplicate error variant; rename it with `Variant(path::Error)`",
                    ));
                }
                tokens.push(quote!(#[display("{_0}")] #name(#[from] #ty)));
            }
            let attrs = data_attrs(err_attrs, true);
            quote!(#attrs pub enum Err { #(#tokens),* })
        };
        let args_attrs = data_attrs(args_attrs, false);
        modules.push(quote! {
            pub mod #ident {
                #[allow(unused_imports)]
                use super::*;
                #(#embedded)*
                #args_attrs
                pub struct Args #args_generics { #(#fields),* }
                #ok_tokens
                #err_tokens
                pub type Output #ok_generics = #result<Ok #ok_generics, Err>;
            }
        });
        methods.push(quote! {
            #(#method_attrs)*
            fn #ident #args_generics (&mut self) -> impl ::eva::handling::CallStep<#ident::Args #args_generics, Output = #ident::Output #ok_generics>;
        });
        clients.push(quote! {
            #(#method_attrs)*
            #[allow(clippy::too_many_arguments)]
            pub fn #ident #args_generics (&mut self, #(#params),*) -> impl ::eva::fut::Fut<Output = #ident::Output #ok_generics> {
                let mut step = self.0.#ident();
                let args = #ident::Args { #(#values),* };
                async move { ::eva::handling::CallStep::call(&mut step, args).await }
            }
        });
        entries.push(quote! {
            #(#[#flags])*
            #ident = #ident #args_generics {
                args: $($root)*::#ident::Args #args_generics,
                ok: $($root)*::#ident::Ok #ok_generics,
                err: $($root)*::#ident::Err,
                output: $($root)*::#ident::Output #ok_generics,
                docs: [#(#doc_strings),*],
            }
        });
    }
    let reexport = if let Some(selected) = opts.export {
        let selected = selected.unwrap_or(method_names);
        let mut seen = HashSet::new();
        for method in &selected {
            if !names.contains(&method.to_string()) {
                return Err(syn::Error::new(
                    method.span(),
                    "unknown service method in export",
                ));
            }
            if !seen.insert(method.to_string()) {
                return Err(syn::Error::new(
                    method.span(),
                    "duplicate service method in export",
                ));
            }
        }
        quote!(#vis use #name::{#(#selected),*};)
    } else {
        quote!()
    };
    let export = unique("service", name);
    Ok(quote! {
        #(#docs)*
        #[allow(non_snake_case)]
        #vis mod #name {
            #[allow(unused_imports)]
            use super::*;
            #(#modules)*
            #[::eva::auto_impl(&mut, Box)]
            pub trait Trait: Send + Sync { #(#methods)* }
            #[derive(Debug, Clone, Copy)]
            #[repr(transparent)]
            pub struct Client<S>(pub S);
            impl<S> Client<S> {
                pub const fn new(inner: S) -> Self { Self(inner) }
                pub fn into_inner(self) -> S { self.0 }
                pub fn inner_mut(&mut self) -> &mut impl Trait where S: Trait { &mut self.0 }
            }
            impl<S: Trait> Client<S> { #(#clients)* }
            #[doc(hidden)]
            #[macro_export]
            macro_rules! #export {
                (@append [$($root:tt)*] [$segment:ident $accessor:ident] [$($callback:tt)*] [$($acc:tt)*] []) => {
                    $($callback)*! {
                        $($acc)*
                        $segment { $accessor: #name {
                            trait: $($root)*::Trait,
                            client: $($root)*::Client,
                            #(#entries)*
                        } }
                    }
                };
                (@append [$($root:tt)*] [$segment:ident $accessor:ident] [$($callback:tt)*] [$($acc:tt)*]
                    [([$($next:tt)*] $next_segment:ident $next_accessor:ident) $($rest:tt)*]) => {
                    $($next)*::for_each_endpoint! {
                        @append [$($next)*] [$next_segment $next_accessor]
                        [$($callback)*] [
                            $($acc)*
                            $segment { $accessor: #name {
                                trait: $($root)*::Trait,
                                client: $($root)*::Client,
                                #(#entries)*
                            } }
                        ] [$($rest)*]
                    }
                };
            }
            pub use #export as for_each_endpoint;
        }
        #reexport
    })
}
