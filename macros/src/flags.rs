use proc_macro2::TokenStream;
use syn::{
    Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

fn list(input: ParseStream) -> syn::Result<Vec<syn::Ident>> {
    let content;
    syn::bracketed!(content in input);
    Ok(
        Punctuated::<syn::Ident, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect(),
    )
}

fn braced(input: ParseStream) -> syn::Result<TokenStream> {
    let content;
    syn::braced!(content in input);
    content.parse()
}

struct Check {
    allowed: Vec<syn::Ident>,
    flags: Vec<syn::Ident>,
}
impl Parse for Check {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let allowed = list(input)?;
        let mut flags = Vec::new();
        while !input.is_empty() {
            flags.push(input.parse()?);
        }
        Ok(Self { allowed, flags })
    }
}

pub fn check(input: TokenStream) -> syn::Result<TokenStream> {
    let Check { allowed, flags } = syn::parse2(input)?;
    let error = flags
        .iter()
        .filter(|flag| !allowed.contains(flag))
        .map(|flag| {
            syn::Error::new(
                flag.span(),
                format!("unknown service flag `{flag}`; declare it in `#[export(flags(...))]`"),
            )
        })
        .reduce(|mut all, error| {
            all.combine(error);
            all
        });
    match error {
        Some(error) => Err(error),
        None => Ok(TokenStream::new()),
    }
}

struct IfFlag {
    flag: syn::Ident,
    flags: Vec<syn::Ident>,
    then: TokenStream,
    otherwise: TokenStream,
}
impl Parse for IfFlag {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let flag = input.parse()?;
        input.parse::<Token![in]>()?;
        let flags = list(input)?;
        let then = braced(input)?;
        let otherwise = if input.parse::<Option<Token![else]>>()?.is_some() {
            braced(input)?
        } else {
            TokenStream::new()
        };
        Ok(Self {
            flag,
            flags,
            then,
            otherwise,
        })
    }
}

pub fn if_flag(input: TokenStream) -> syn::Result<TokenStream> {
    let IfFlag {
        flag,
        flags,
        then,
        otherwise,
    } = syn::parse2(input)?;
    Ok(if flags.contains(&flag) {
        then
    } else {
        otherwise
    })
}
