use std::collections::HashSet;

use proc_macro2::{Span, TokenStream, TokenTree};
use quote::ToTokens;

use crate::utils::Errors;

pub struct Params {
    pub state: syn::PatType,
    pub args: syn::PatType,
}

/// Checks that `item` can become `Endpoint::call`, reporting every violation.
pub fn item(item: &syn::ItemFn) -> syn::Result<Params> {
    let sig = &item.sig;
    let mut errors = Errors::default();
    attrs(&item.attrs, &mut errors);
    qualifiers(sig, &mut errors);
    let params = params(sig, &mut errors);
    if let syn::ReturnType::Type(_, ty) = &sig.output {
        let problem =
            "`impl Trait` is not allowed in the endpoint return type (use a concrete type)";
        no_impl_trait(ty, problem, &mut errors);
    }
    if let Some(params) = &params {
        constrained_generics(&sig.generics, params, &mut errors);
    }

    errors.finish()?;
    Ok(params.expect("malformed parameters are reported as errors"))
}

fn message(problem: &str) -> String {
    format!("{problem}; expected `async fn Name(state: S, args: A) -> O`")
}

fn error(tokens: impl ToTokens, problem: &str) -> syn::Error {
    syn::Error::new_spanned(tokens, message(problem))
}

/// Attributes that `Endpoint::call` would receive without doing what they promise for the fn.
fn attrs(attrs: &[syn::Attribute], errors: &mut Errors) {
    for attr in attrs {
        let name = attr.path().get_ident().map(ToString::to_string);
        let msg = match name.as_deref() {
            Some("unsafe" | "no_mangle" | "export_name" | "link_section") => {
                "endpoint fn cannot take unsafe attributes: `#[endpoint]` turns it into \
                 `Endpoint::call`"
            }
            Some("must_use") => "`#[must_use]` has no effect on `Endpoint::call`",
            _ => continue,
        };
        errors.push(syn::Error::new_spanned(attr, msg));
    }
}

fn qualifiers(sig: &syn::Signature, errors: &mut Errors) {
    if let Some(constness) = &sig.constness {
        errors.push(error(constness, "endpoint fn cannot be `const`"));
    }
    if let Some(unsafety) = &sig.unsafety {
        let problem = "endpoint fn cannot be `unsafe` because `Endpoint::call` is safe";
        errors.push(error(unsafety, problem));
    }
    if let Some(abi) = &sig.abi {
        errors.push(error(abi, "endpoint fn cannot have an ABI"));
    }
    if sig.asyncness.is_none() {
        errors.push(error(sig.fn_token, "endpoint fn must be `async`"));
    }
}

fn params(sig: &syn::Signature, errors: &mut Errors) -> Option<Params> {
    let mut typed = Vec::new();
    for input in &sig.inputs {
        match input {
            syn::FnArg::Typed(param) => {
                let problem =
                    "`impl Trait` is not allowed in endpoint parameters (use a generic parameter)";
                no_impl_trait(&param.ty, problem, errors);
                typed.push(param);
            }
            syn::FnArg::Receiver(receiver) => {
                errors.push(error(receiver, "endpoint fn cannot take `self`"));
            }
        }
    }
    if let Some(variadic) = &sig.variadic {
        errors.push(error(variadic, "endpoint fn cannot be variadic"));
    }

    let count = sig.inputs.len();
    if count != 2 {
        let problem = format!("endpoint fn takes 2 parameters, state and args, but has {count}");
        errors.push(match count {
            0 => syn::Error::new(sig.paren_token.span.join(), message(&problem)),
            _ => error(&sig.inputs, &problem),
        });
    }

    match typed[..] {
        [state, args] => Some(Params {
            state: state.clone(),
            args: args.clone(),
        }),
        _ => None,
    }
}

fn no_impl_trait(ty: &syn::Type, problem: &str, errors: &mut Errors) {
    if let Some(span) = impl_keyword(ty.to_token_stream()) {
        errors.push(syn::Error::new(span, message(problem)));
    }
}

fn impl_keyword(tokens: TokenStream) -> Option<Span> {
    tokens.into_iter().find_map(|token| match token {
        TokenTree::Ident(ident) if ident == "impl" => Some(ident.span()),
        TokenTree::Group(group) => impl_keyword(group.stream()),
        _ => None,
    })
}

/// `impl<generics> Endpoint<A, S> for Name` leaves a generic parameter that is absent from `A`
/// and `S` unconstrained (E0207).
fn constrained_generics(generics: &syn::Generics, params: &Params, errors: &mut Errors) {
    let mut names = HashSet::new();
    for param in [&params.state, &params.args] {
        type_names(&param.ty, &mut names);
    }

    for param in &generics.params {
        let (name, tokens) = match param {
            syn::GenericParam::Type(ty) => (ty.ident.to_string(), ty.ident.to_token_stream()),
            syn::GenericParam::Const(c) => (c.ident.to_string(), c.ident.to_token_stream()),
            syn::GenericParam::Lifetime(lt) => {
                (lt.lifetime.to_string(), lt.lifetime.to_token_stream())
            }
        };
        if !names.contains(&name) {
            let problem =
                format!("generic parameter `{name}` must appear in the state or args type");
            errors.push(error(tokens, &problem));
        }
    }
}

/// Adds the names that `ty` may constrain: projections (`T::Assoc`, `<T as Trait>::Assoc`)
/// constrain nothing, and what is not understood counts in full, so no valid signature is rejected.
fn type_names(ty: &syn::Type, names: &mut HashSet<String>) {
    match ty {
        syn::Type::Path(ty) if ty.qself.is_none() => path_names(&ty.path, names),
        syn::Type::Path(_) => {}
        syn::Type::Array(ty) => {
            type_names(&ty.elem, names);
            token_names(ty.len.to_token_stream(), names);
        }
        syn::Type::BareFn(ty) => {
            ty.inputs.iter().for_each(|arg| type_names(&arg.ty, names));
            return_names(&ty.output, names);
        }
        syn::Type::Group(ty) => type_names(&ty.elem, names),
        syn::Type::Paren(ty) => type_names(&ty.elem, names),
        syn::Type::Ptr(ty) => type_names(&ty.elem, names),
        syn::Type::Slice(ty) => type_names(&ty.elem, names),
        syn::Type::Reference(ty) => {
            names.extend(ty.lifetime.as_ref().map(ToString::to_string));
            type_names(&ty.elem, names);
        }
        syn::Type::ImplTrait(ty) => bound_names(&ty.bounds, names),
        syn::Type::TraitObject(ty) => bound_names(&ty.bounds, names),
        syn::Type::Tuple(ty) => ty.elems.iter().for_each(|ty| type_names(ty, names)),
        other => token_names(other.to_token_stream(), names),
    }
}

/// Only a single-ident path can name a parameter: in `T::Assoc` and `module::T` neither `T` does.
fn path_names(path: &syn::Path, names: &mut HashSet<String>) {
    names.extend(path.get_ident().map(ToString::to_string));
    for segment in &path.segments {
        match &segment.arguments {
            syn::PathArguments::None => {}
            syn::PathArguments::AngleBracketed(args) => {
                for arg in &args.args {
                    match arg {
                        syn::GenericArgument::Type(ty) => type_names(ty, names),
                        syn::GenericArgument::AssocType(assoc) => type_names(&assoc.ty, names),
                        other => token_names(other.to_token_stream(), names),
                    }
                }
            }
            syn::PathArguments::Parenthesized(args) => {
                args.inputs.iter().for_each(|ty| type_names(ty, names));
                return_names(&args.output, names);
            }
        }
    }
}

fn return_names(output: &syn::ReturnType, names: &mut HashSet<String>) {
    if let syn::ReturnType::Type(_, ty) = output {
        type_names(ty, names);
    }
}

fn bound_names<'a>(
    bounds: impl IntoIterator<Item = &'a syn::TypeParamBound>,
    names: &mut HashSet<String>,
) {
    for bound in bounds {
        match bound {
            syn::TypeParamBound::Trait(bound) => path_names(&bound.path, names),
            other => token_names(other.to_token_stream(), names),
        }
    }
}

/// Adds every ident and lifetime in `tokens`.
fn token_names(tokens: TokenStream, names: &mut HashSet<String>) {
    let mut tokens = tokens.into_iter();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Group(group) => token_names(group.stream(), names),
            TokenTree::Punct(punct) if punct.as_char() == '\'' => {
                if let Some(TokenTree::Ident(ident)) = tokens.next() {
                    names.insert(format!("'{ident}"));
                }
            }
            TokenTree::Ident(ident) => {
                names.insert(ident.to_string());
            }
            _ => {}
        }
    }
}
