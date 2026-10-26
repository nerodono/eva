//! `#[int(newtype)]`: a struct over one `Integer` field, created from and unpacked to the
//! field's primitive.

use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use quote::{ToTokens as _, format_ident, quote, quote_spanned};
use syn::{
    Token,
    parse::{ParseStream, Parser as _},
    token,
};

use crate::{
    docs,
    opts::{self, CommonTraits, Opts, Table},
    utils::{self, Errors, maybe},
};

const OPTIONS: Table = Table {
    keys: &[opts::CRATE],
    not: &[opts::COMMON_NOT, &["Display"]],
};

const CONDITION_EXPECTED: &str = "expected a condition over `n`, as in `newtype(n > 5)`";

/// Whether `args` start with the `newtype` keyword, which a `macro_rules!` fragment wraps in an
/// invisible group.
pub fn applies(args: &TokenStream) -> bool {
    match args.clone().into_iter().next() {
        Some(TokenTree::Ident(ident)) => ident == "newtype",
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::None => {
            applies(&group.stream())
        }
        _ => false,
    }
}

/// The span of the first `name` identifier in `tokens`, through invisible groups.
fn find_ident(tokens: TokenStream, name: &str) -> Option<Span> {
    tokens.into_iter().find_map(|tree| match tree {
        TokenTree::Ident(ident) if ident == name => Some(ident.span()),
        TokenTree::Group(group) => find_ident(group.stream(), name),
        _ => None,
    })
}

pub fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let (item, args) = utils::both(parse_item(item), Args::parse.parse2(args))?;
    let newtype = Newtype { item, args };

    let definition = newtype.definition();
    let inherent = newtype.inherent();
    let traits = newtype.traits();
    Ok(quote! {
        #definition
        #inherent
        #traits
    })
}

struct Args {
    condition: Option<syn::Expr>,
    eva: syn::Path,
    traits: CommonTraits,
    display: bool,
}

impl Args {
    /// `newtype`, an optional `(condition)` and the options.
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<syn::Ident>()?;
        if input.peek(Token![,]) && input.peek2(token::Paren) {
            let msg = "the condition follows `newtype` directly, as in `newtype(n > 5)`";
            return Err(syn::Error::new(input.span(), msg));
        }
        let condition = if input.peek(token::Paren) {
            Some(Self::condition(input)?)
        } else {
            None
        };
        let opts = Opts::parse_after_positional(input, &OPTIONS)?;

        Ok(Self {
            condition,
            eva: opts.krate(),
            traits: opts.common(),
            display: opts.not("Display").is_none(),
        })
    }

    fn condition(input: ParseStream) -> syn::Result<syn::Expr> {
        let content;
        let paren = syn::parenthesized!(content in input);
        if content.is_empty() {
            return Err(syn::Error::new(paren.span.join(), CONDITION_EXPECTED));
        }
        let condition = content.parse::<syn::Expr>()?;
        if !content.is_empty() {
            return Err(content.error("unexpected token"));
        }
        if let Some(span) = find_ident(condition.to_token_stream(), "self") {
            let msg = "the condition sees the value as `n`; there is no `self` yet";
            return Err(syn::Error::new(span, msg));
        }
        Ok(condition)
    }
}

struct Item {
    inner: syn::ItemStruct,
    field: syn::Field,
    member: syn::Member,
}

/// A struct with one field, not generic and without a `#[repr]`, which the macro adds.
fn parse_item(item: TokenStream) -> syn::Result<Item> {
    let input = syn::parse2::<syn::DeriveInput>(item)?;
    let requires_struct = |span| syn::Error::new(span, "`newtype` requires a struct");
    let data = match input.data {
        syn::Data::Struct(data) => data,
        syn::Data::Enum(data) => return Err(requires_struct(data.enum_token.span)),
        syn::Data::Union(data) => return Err(requires_struct(data.union_token.span)),
    };

    let mut errors = Errors::default();
    let mut fields = data.fields.iter().cloned();
    let field = match (fields.next(), fields.next()) {
        (Some(field), None) => Some(field),
        _ => {
            let msg = "`newtype` requires exactly one field";
            errors.push(match &data.fields {
                syn::Fields::Unit => syn::Error::new_spanned(&input.ident, msg),
                fields => syn::Error::new_spanned(fields, msg),
            });
            None
        }
    };
    if !input.generics.params.is_empty() {
        let msg = "#[int] types cannot be generic";
        errors.push(syn::Error::new_spanned(&input.generics, msg));
    }
    if let Some(where_clause) = &input.generics.where_clause {
        let msg = "#[int] types cannot have a where clause";
        errors.push(syn::Error::new_spanned(where_clause, msg));
    }
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("repr"))
    {
        let msg = "#[int(newtype)] adds `#[repr(transparent)]`; remove this `#[repr]`";
        errors.push(syn::Error::new_spanned(attr, msg));
    }
    errors.finish()?;

    let field = field.expect("a missing field is reported as an error");
    let member = match &field.ident {
        Some(name) => syn::Member::Named(name.clone()),
        None => syn::Member::Unnamed(syn::Index::from(0)),
    };
    Ok(Item {
        inner: syn::ItemStruct {
            attrs: input.attrs,
            vis: input.vis,
            struct_token: data.struct_token,
            ident: input.ident,
            generics: input.generics,
            fields: data.fields,
            semi_token: data.semi_token,
        },
        field,
        member,
    })
}

struct Newtype {
    item: Item,
    args: Args,
}

impl Newtype {
    fn ident(&self) -> &syn::Ident {
        &self.item.inner.ident
    }

    fn inner(&self) -> &syn::Type {
        &self.item.field.ty
    }

    fn int(&self, name: &str) -> TokenStream {
        let eva = &self.args.eva;
        let name = format_ident!("{name}");
        quote!(#eva::int::#name)
    }

    /// `<Inner as Integer>::Prim`.
    fn prim(&self) -> TokenStream {
        let inner = self.inner();
        let integer = self.int("Integer");
        quote!(<#inner as #integer>::Prim)
    }

    /// `Self(inner)` or `Self { field: inner }`.
    fn construct(&self, inner: &syn::Ident) -> TokenStream {
        match &self.item.field.ident {
            Some(field) => quote_spanned!(Span::mixed_site()=> Self { #field: #inner }),
            None => quote_spanned!(Span::mixed_site()=> Self(#inner)),
        }
    }

    fn definition(&self) -> TokenStream {
        let item = &self.item.inner;
        // Derived, so consts of the type work as patterns.
        quote! {
            #[repr(transparent)]
            #[derive(
                ::core::clone::Clone,
                ::core::marker::Copy,
                ::core::cmp::PartialEq,
                ::core::cmp::Eq,
                ::core::cmp::PartialOrd,
                ::core::cmp::Ord,
                ::core::hash::Hash,
            )]
            #item
        }
    }

    fn inherent(&self) -> TokenStream {
        let name = self.ident();
        let vis = &self.item.inner.vis;
        let inner = self.inner();
        let integer = self.int("Integer");
        let prim = self.prim();
        let member = &self.item.member;
        let wrapped = utils::local("inner");
        let construct = self.construct(&wrapped);

        let new = maybe(self.args.condition.is_none(), || {
            quote_spanned! {Span::mixed_site()=>
                /// Wraps `inner`.
                #vis const fn new(inner: #inner) -> Self {
                    let #wrapped = inner;
                    #construct
                }
            }
        });
        // Checked after the field, so the condition sees only accepted values; the binder takes
        // the hygiene of the user's `n`, which a `$cond` fragment keeps from its caller.
        let check = self.args.condition.as_ref().map(|condition| {
            let span = find_ident(condition.to_token_stream(), "n").unwrap_or_else(Span::call_site);
            let n = syn::Ident::new("n", span);
            quote_spanned! {Span::mixed_site()=>
                let #n = prim;
                let __ok: ::core::primitive::bool = #condition;
                if !__ok {
                    return ::core::option::Option::None;
                }
            }
        });
        let try_new_doc = match &self.args.condition {
            Some(_) => "`prim` as `Self`, if the field accepts it and the condition holds.",
            None => "`prim` as `Self`, if the field accepts it.",
        };
        let unchecked_doc = match &self.args.condition {
            Some(_) => "The field must accept `prim`; the condition is not checked.",
            None => "The field must accept `prim`.",
        };

        quote_spanned! {Span::mixed_site()=>
            impl #name {
                #new

                #[doc = #try_new_doc]
                #vis fn try_new(prim: #prim) -> ::core::option::Option<Self> {
                    let #wrapped = match <#inner as #integer>::try_new(prim) {
                        ::core::option::Option::Some(#wrapped) => #wrapped,
                        ::core::option::Option::None => return ::core::option::Option::None,
                    };
                    #check
                    ::core::option::Option::Some(#construct)
                }

                /// `prim` as `Self`, unchecked.
                ///
                /// # Safety
                ///
                #[doc = #unchecked_doc]
                #vis unsafe fn new_unchecked(prim: #prim) -> Self {
                    let #wrapped = unsafe { <#inner as #integer>::new_unchecked(prim) };
                    #construct
                }

                /// The field.
                #vis const fn inner(&self) -> #inner {
                    self.#member
                }

                /// The value as its primitive.
                #vis fn prim(&self) -> #prim {
                    <#inner as #integer>::prim(&self.#member)
                }
            }
        }
    }

    fn traits(&self) -> TokenStream {
        let traits = &self.args.traits;
        let integer = self.integer();
        let infallible = maybe(self.args.condition.is_none(), || self.infallible());
        let cmp = self.cmp();
        let debug = maybe(traits.debug, || self.fmt("Debug"));
        let display = maybe(self.args.display, || self.fmt("Display"));
        let serialize = maybe(traits.serialize, || self.serialize());
        let deserialize = maybe(traits.deserialize, || self.deserialize());
        let json_schema = maybe(traits.json_schema, || self.json_schema());

        quote! {
            #integer
            #infallible
            #cmp
            #debug
            #display
            #serialize
            #deserialize
            #json_schema
        }
    }

    fn integer(&self) -> TokenStream {
        let name = self.ident();
        let inner = self.inner();
        let integer = self.int("Integer");
        let prim = self.prim();
        let expecting = match &self.args.condition {
            Some(condition) => {
                let eva = &self.args.eva;
                let text = quote!(#condition).to_string();
                quote_spanned! {Span::mixed_site()=>
                    #eva::int::expecting_where(__f, <#inner as #integer>::expecting, #text)
                }
            }
            None => quote_spanned! {Span::mixed_site()=>
                <#inner as #integer>::expecting(__f)
            },
        };

        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #integer for #name {
                type Inner = #inner;
                type Prim = #prim;

                #[inline]
                fn try_new(__prim: #prim) -> ::core::option::Option<Self> {
                    Self::try_new(__prim)
                }

                #[inline]
                unsafe fn new_unchecked(__prim: #prim) -> Self {
                    unsafe { Self::new_unchecked(__prim) }
                }

                #[inline]
                fn inner(&self) -> #inner {
                    Self::inner(self)
                }

                #[inline]
                fn prim(&self) -> #prim {
                    Self::prim(self)
                }

                fn expecting(__f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    #expecting
                }
            }
        }
    }

    /// Without a condition, wrapping the field cannot fail.
    fn infallible(&self) -> TokenStream {
        let name = self.ident();
        let inner = self.inner();
        let infallible = self.int("InfallibleInteger");
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #infallible for #name {
                #[inline]
                fn new(__inner: #inner) -> Self {
                    Self::new(__inner)
                }
            }

            #[automatically_derived]
            impl ::core::convert::From<#inner> for #name {
                #[inline]
                fn from(__inner: #inner) -> Self {
                    Self::new(__inner)
                }
            }
        }
    }

    /// `PartialEq<Prim>` through a bound rather than a projection, which coherence cannot
    /// normalize for a foreign field type.
    fn cmp(&self) -> TokenStream {
        let name = self.ident();
        let inner = self.inner();
        let integer = self.int("Integer");
        let primitive = self.int("Primitive");
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl<__P: #primitive> ::core::cmp::PartialEq<__P> for #name
            where
                #inner: #integer<Prim = __P>,
            {
                #[inline]
                fn eq(&self, __other: &__P) -> ::core::primitive::bool {
                    Self::prim(self) == *__other
                }
            }

            #[automatically_derived]
            impl ::core::convert::From<#name> for #inner {
                #[inline]
                fn from(__value: #name) -> Self {
                    #name::inner(&__value)
                }
            }
        }
    }

    /// `Debug` or `Display`, as the number.
    fn fmt(&self, trait_name: &str) -> TokenStream {
        let name = self.ident();
        let prim = self.prim();
        let fmt_trait = format_ident!("{trait_name}");
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl ::core::fmt::#fmt_trait for #name {
                fn fmt(&self, __f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    <#prim as ::core::fmt::#fmt_trait>::fmt(&Self::prim(self), __f)
                }
            }
        }
    }

    fn serialize(&self) -> TokenStream {
        let name = self.ident();
        let prim = self.prim();
        let eva = &self.args.eva;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #eva::_priv::serde::Serialize for #name {
                fn serialize<__S: #eva::_priv::serde::Serializer>(
                    &self,
                    __serializer: __S,
                ) -> ::core::result::Result<__S::Ok, __S::Error> {
                    <#prim as #eva::_priv::serde::Serialize>::serialize(&Self::prim(self), __serializer)
                }
            }
        }
    }

    fn deserialize(&self) -> TokenStream {
        let name = self.ident();
        let eva = &self.args.eva;
        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl<'de> #eva::_priv::serde::Deserialize<'de> for #name {
                fn deserialize<__D: #eva::_priv::serde::Deserializer<'de>>(
                    __deserializer: __D,
                ) -> ::core::result::Result<Self, __D::Error> {
                    #eva::int::deserialize(__deserializer)
                }
            }
        }
    }

    /// The field's schema, with the doc comment as `description` in place of the field's.
    fn json_schema(&self) -> TokenStream {
        let name = self.ident();
        let schema_name = name.to_string();
        let inner = self.inner();
        let schemars = {
            let eva = &self.args.eva;
            quote!(#eva::_priv::schemars)
        };
        let description = match docs::description(&self.item.inner.attrs) {
            Some(description) => quote_spanned! {Span::mixed_site()=>
                #schemars::Schema::insert(
                    &mut __schema,
                    ::std::borrow::ToOwned::to_owned("description"),
                    ::core::convert::Into::into(#description),
                );
            },
            None => quote_spanned! {Span::mixed_site()=>
                #schemars::Schema::remove(&mut __schema, "description");
            },
        };

        quote_spanned! {Span::mixed_site()=>
            #[automatically_derived]
            impl #schemars::JsonSchema for #name {
                fn schema_name() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(#schema_name)
                }

                fn schema_id() -> ::std::borrow::Cow<'static, ::core::primitive::str> {
                    ::std::borrow::Cow::Borrowed(::core::concat!(::core::module_path!(), "::", #schema_name))
                }

                fn json_schema(__generator: &mut #schemars::SchemaGenerator) -> #schemars::Schema {
                    let mut __schema = <#inner as #schemars::JsonSchema>::json_schema(__generator);
                    #description
                    __schema
                }
            }
        }
    }
}
