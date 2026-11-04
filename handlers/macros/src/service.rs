use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream, Parser},
    spanned::Spanned,
    visit_mut::VisitMut,
};

use crate::{
    docs,
    opts::{self, Key, Opts, Table},
    utils::{comma_list, unique_ident},
};

const OPTIONS: Table = Table {
    keys: &[
        opts::CRATE,
        Key::optional_list("export"),
        Key::value::<syn::Path>("result"),
        Key::value::<syn::Type>("error"),
    ],
    not: &[],
};

struct Options {
    eva: syn::Path,
    export: Option<Option<Vec<syn::Ident>>>,
    result: Option<syn::Path>,
    error: Option<syn::Type>,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let opts = Opts::parse(input, &OPTIONS)?;
        Ok(Self {
            eva: opts.krate().unwrap_or_else(crate::runtime),
            export: opts.optional_list("export", comma_list)?,
            result: opts.value("result"),
            error: opts.value("error"),
        })
    }
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

/// Route `data` attributes through the selected runtime, tracking whether they derive serde.
/// Error types always get `error`.
fn data_attrs(
    attrs: Vec<syn::Attribute>,
    error: bool,
    data: &syn::Path,
) -> syn::Result<(TokenStream, bool)> {
    let mut has_data = false;
    let mut serde = true;
    let mut output = Vec::with_capacity(attrs.len());
    for mut attr in attrs {
        if attr.path().is_ident("data") {
            has_data = true;
            let mut tokens = match &attr.meta {
                syn::Meta::Path(_) => TokenStream::new(),
                syn::Meta::List(list) => list.tokens.clone(),
                syn::Meta::NameValue(_) => {
                    return Err(syn::Error::new(attr.span(), "expected `data(...)`"));
                }
            };
            let args = syn::parse2::<eva_macros_common::data_options::Args>(tokens.clone())?;
            serde &= args.serde();
            if error && !args.error {
                tokens = if tokens.is_empty() {
                    quote!(error)
                } else {
                    quote!(error, #tokens)
                };
            }
            if args.eva.is_none() {
                tokens = quote!(crate = #data, #tokens);
            }
            attr.meta = syn::parse_quote!(#data::data(#tokens));
        }
        output.push(attr);
    }
    let default = (!has_data).then(|| {
        if error {
            quote!(#[#data::data(crate = #data, error)])
        } else {
            quote!(#[#data::data(crate = #data)])
        }
    });
    Ok((quote!(#default #(#output)*), serde))
}

fn attributes(attr: &syn::Attribute) -> syn::Result<Vec<syn::Attribute>> {
    let metas = attr.parse_args_with(comma_list::<syn::Meta>)?;
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
    let eva = &opts.eva;
    let data: syn::Path = syn::parse_quote!(#eva::_priv::data);
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
    // Resolve the configured constructor before introducing the service's own `Result`.
    let result_import = unique_ident("result", name.span());
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
            embedded.extend(syn::parse2::<syn::File>(block.mac.tokens.clone())?.items);
            continue;
        }
        return Err(syn::Error::new(
            entry.span(),
            "expected a service method or embed! { ... }",
        ));
    }
    for (method, embedded) in declarations {
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
        let mut error_ty = opts.error.clone();
        let mut args_attrs = Vec::new();
        let mut ok_attrs = Vec::new();
        let mut err_attrs = Vec::new();
        let mut method_attrs = Vec::new();
        for attr in &method.attrs {
            if attr.path().is_ident("flags") {
                flags.extend(attr.parse_args_with(comma_list::<syn::Ident>)?);
            } else if attr.path().is_ident("errors") {
                if errors.is_some() {
                    return Err(syn::Error::new(attr.span(), "duplicate errors attribute"));
                }
                errors = Some(attr.parse_args_with(comma_list::<ErrorVariant>)?);
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
        let doc_strings: Vec<_> = docs::exprs(&method_attrs).collect();
        let mut lifetimes = Lifetimes {
            names: Vec::new(),
            infer: true,
        };
        let mut fields = Vec::new();
        let mut params = Vec::new();
        let mut values = Vec::new();
        let mut field_names = HashSet::new();
        for arg in &sig.inputs {
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
            let attrs = &arg.attrs;
            fields.push(quote!(#(#attrs)* pub #field: #ty));
            params.push(quote!(#field: #ty));
            values.push(field);
        }
        let args_generics = generics(&lifetimes.names);
        let mut ok_lifetimes = Lifetimes {
            names: Vec::new(),
            infer: false,
        };
        let mut ty = match &sig.output {
            syn::ReturnType::Type(_, ty) => (**ty).clone(),
            _ => syn::parse_quote!(()),
        };
        let ok_tokens = match &mut ty {
            syn::Type::Macro(m) if m.mac.path.is_ident("Struct") || m.mac.path.is_ident("Enum") => {
                if !matches!(m.mac.delimiter, syn::MacroDelimiter::Brace(_)) {
                    return Err(syn::Error::new_spanned(
                        &m.mac,
                        "expected Struct! { ... } or Enum! { ... }",
                    ));
                }
                let parser = |input: ParseStream| {
                    let attrs = syn::Attribute::parse_inner(input)?;
                    let body = input.parse::<TokenStream>()?;
                    Ok((attrs, body))
                };
                let (attrs, body) = parser.parse2(m.mac.tokens.clone())?;
                ok_attrs.extend(attrs.into_iter().map(|mut attr| {
                    attr.style = syn::AttrStyle::Outer;
                    attr
                }));
                let definition = if m.mac.path.is_ident("Struct") {
                    quote!(pub struct Ok { #body })
                } else {
                    quote!(pub enum Ok { #body })
                };
                let mut definition: syn::DeriveInput = syn::parse2(definition)?;
                match &mut definition.data {
                    syn::Data::Struct(data) => {
                        // Service fields are public without requiring callers to spell `pub`.
                        for field in &mut data.fields {
                            field.vis = syn::parse_quote!(pub);
                        }
                        ok_lifetimes.visit_fields_mut(&mut data.fields);
                    }
                    syn::Data::Enum(data) => {
                        for variant in &mut data.variants {
                            ok_lifetimes.visit_fields_mut(&mut variant.fields);
                        }
                    }
                    syn::Data::Union(_) => unreachable!(),
                }
                definition.generics = syn::parse2(generics(&ok_lifetimes.names))?;
                let (attrs, _) = data_attrs(ok_attrs, false, &data)?;
                quote!(#attrs #definition)
            }
            ty => {
                ok_lifetimes.visit_type_mut(ty);
                let ok_generics = generics(&ok_lifetimes.names);
                let custom_serde = ok_attrs.iter().any(|a| a.path().is_ident("serde"));
                let (attrs, serde) = data_attrs(ok_attrs, false, &data)?;
                let transparent = (serde && !custom_serde).then(|| quote!(#[serde(transparent)]));
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
            let (attrs, _) = data_attrs(err_attrs, true, &data)?;
            quote!(#attrs pub enum Err { #(#tokens),* })
        };
        let (args_attrs, _) = data_attrs(args_attrs, false, &data)?;
        let endpoint_scope = crate::endpoint::scope::export(ident, &ok_lifetimes.names);
        modules.push(quote! {
            pub mod #ident {
                #[allow(unused_imports)]
                use super::*;
                #[allow(unused_imports)]
                use super::#ident as this;
                #(#embedded)*
                #args_attrs
                pub struct Args #args_generics { #(#fields),* }
                #ok_tokens
                #err_tokens
                #endpoint_scope
            }
        });
        let output = quote!(self::Result<#ident::Ok #ok_generics, #ident::Err>);
        methods.push(quote! {
            #(#method_attrs)*
            fn #ident #args_generics (&mut self) -> impl #eva::_priv::CallStep<#ident::Args #args_generics, Output = #output>;
        });
        clients.push(quote! {
            // An import per method resolves `this` in its signature, including macro inputs.
            // The anonymous const adds no module level, preserving `self` and `super` paths.
            const _: () = {
                #[allow(unused_imports)]
                use self::#ident as this;
                impl<S: Trait> Client<S> {
                    #(#method_attrs)*
                    #[allow(clippy::too_many_arguments)]
                    pub fn #ident #args_generics (&mut self, #(#params),*) -> impl ::core::future::Future<Output = #output> + Send {
                        let mut step = self.0.#ident();
                        let args = #ident::Args { #(#values),* };
                        async move { #eva::_priv::CallStep::call(&mut step, args).await }
                    }
                }
            };
        });
        let lifetime_names = &lifetimes.names;
        entries.push(quote! {
            #ident {
                flags: [#(#flags),*],
                lifetimes: [#(#lifetime_names),*],
                args: $($root)*::#ident::Args #args_generics,
                ok: $($root)*::#ident::Ok #ok_generics,
                err: $($root)*::#ident::Err,
                output: $($root)*::Result<$($root)*::#ident::Ok #ok_generics, $($root)*::#ident::Err>,
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
    let service_trait = eva_macros_common::auto_impl::expand(
        quote!(&mut, Box),
        quote!(pub trait Trait: Send + Sync { #(#methods)* }),
    )?;
    let export = unique_ident("service", name.span());
    Ok(quote! {
        use #result as #result_import;
        #(#docs)*
        #[allow(non_snake_case)]
        #vis mod #name {
            #[allow(unused_imports)]
            use super::*;
            pub type Result<T, E> = #result_import<T, E>;
            #(#modules)*
            #service_trait
            #[derive(Debug, Clone, Copy)]
            #[repr(transparent)]
            pub struct Client<S>(pub S);
            impl<S> Client<S> {
                pub const fn new(inner: S) -> Self { Self(inner) }
                pub fn into_inner(self) -> S { self.0 }
                pub fn inner_mut(&mut self) -> &mut impl Trait where S: Trait { &mut self.0 }
            }
            #(#clients)*
            #[doc(hidden)]
            #[macro_export]
            macro_rules! #export {
                (@append [$($root:tt)*] [$accessor:ident] [$($callback:tt)*] [$($acc:tt)*] []) => {
                    $($callback)*! {
                        $($acc)*
                        $accessor {
                            trait: $($root)*::Trait,
                            client: $($root)*::Client,
                            methods: { #(#entries)* },
                        }
                    }
                };
                (@append [$($root:tt)*] [$accessor:ident] [$($callback:tt)*] [$($acc:tt)*]
                    [([$($next:tt)*] $next_accessor:ident) $($rest:tt)*]) => {
                    $($next)*::for_each_endpoint! {
                        @append [$($next)*] [$next_accessor]
                        [$($callback)*] [
                            $($acc)*
                            $accessor {
                                trait: $($root)*::Trait,
                                client: $($root)*::Client,
                                methods: { #(#entries)* },
                            }
                        ] [$($rest)*]
                    }
                };
            }
            pub use #export as for_each_endpoint;
        }
        #reexport
    })
}
