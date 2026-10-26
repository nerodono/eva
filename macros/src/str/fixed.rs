use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote, quote_spanned};
use syn::spanned::Spanned as _;

use crate::{
    docs,
    utils::{self, Errors, join, local, maybe},
};

use super::{KindImpls, Target, args::Args, construct, requires_struct};

pub fn expand<'a>(
    input: &'a syn::DeriveInput,
    error: &syn::Type,
    args: &Args,
) -> syn::Result<KindImpls<'a>> {
    let (fields, ()) = utils::both(requires_struct(input, "fixed"), reject_repr(&input.attrs))?;
    let eva = &args.eva;
    let target = Target {
        ident: &input.ident,
        generics: bounded_generics(&input.generics, fields, error, eva),
    };
    let layout = Layout::new(input, fields, eva);

    let schema = maybe(args.traits.json_schema, || json_schema(&target, input, eva));
    let mut impls = layout.definition_check();
    impls.extend([
        schema,
        pattern(&target, fields, eva),
        fixed_utf8(&target, eva),
        as_str(&target, &layout, eva),
        from_str(&target, &layout, fields, error, eva),
    ]);

    Ok(KindImpls {
        target,
        attrs: quote! { #[repr(C)] },
        impls,
    })
}

fn reject_repr(attrs: &[syn::Attribute]) -> syn::Result<()> {
    let mut errors = Errors::default();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("repr")) {
        let msg = "`fixed` lays the fields out with `#[repr(C)]` itself; remove this attribute";
        errors.push(syn::Error::new_spanned(attr, msg));
    }
    errors.finish()
}

/// The item's generics plus, per field, `Field: FixedUtf8` and `Error: From<Field::Err>`.
fn bounded_generics(
    generics: &syn::Generics,
    fields: &syn::Fields,
    error: &syn::Type,
    eva: &syn::Path,
) -> syn::Generics {
    let mut generics = generics.clone();
    let predicates = &mut generics.make_where_clause().predicates;
    for ty in fields.iter().map(|field| &field.ty) {
        let eva = located_at(eva, ty.span());
        predicates.push(syn::parse_quote_spanned! {ty.span()=> #ty: #eva::str::FixedUtf8});
        predicates.push(syn::parse_quote_spanned! {ty.span()=>
            #error: ::core::convert::From<<#ty as ::core::str::FromStr>::Err>
        });
    }
    generics
}

/// `as_str` reads every byte, so there must be no padding: under `#[repr(C)]`, fields of
/// alignment 1; checked at definition, or on use for generic types, whose layout is unknown.
struct Layout {
    generic: bool,
    asserts: TokenStream,
}

impl Layout {
    fn new(input: &syn::DeriveInput, fields: &syn::Fields, eva: &syn::Path) -> Self {
        let generic = !input.generics.params.is_empty();
        let ident = &input.ident;
        let ty = if generic {
            quote! { Self }
        } else {
            quote! { #ident }
        };
        let field_tys: Vec<_> = fields.iter().map(|field| &field.ty).collect();
        let aligned = field_tys.iter().map(|field_ty| {
            let eva = located_at(eva, field_ty.span());
            quote_spanned! {field_ty.span()=>
                ::core::assert!(
                    #eva::str::fixed_align::<#field_ty>() == 1,
                    "every field of a `fixed` string must have alignment 1",
                );
            }
        });
        let asserts = quote! {
            #(#aligned)*
            ::core::assert!(
                ::core::mem::size_of::<#ty>() == 0 #(+ ::core::mem::size_of::<#field_tys>())*,
                "a `fixed` string must have no padding",
            );
        };
        Self { generic, asserts }
    }

    fn definition_check(&self) -> TokenStream {
        let asserts = &self.asserts;
        maybe(!self.generic, || quote! { const _: () = { #asserts }; })
    }

    fn use_check(&self) -> TokenStream {
        let asserts = &self.asserts;
        maybe(self.generic, || quote! { const { #asserts } })
    }
}

fn json_schema(target: &Target, input: &syn::DeriveInput, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let ident = &input.ident;
    let schemars = join(&join(eva, "_priv"), "schemars");
    let cow = quote! { ::std::borrow::Cow<'static, ::core::primitive::str> };
    // Instantiations of a generic type differ in length and pattern, so each needs its own id.
    let schema_id = if input.generics.params.is_empty() {
        quote! { ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#ident)) }
    } else {
        quote! { ::core::any::type_name::<Self>() }
    };
    let description = docs::schema_description(&input.attrs);
    let len = local("len");

    quote! {
        #[automatically_derived]
        impl #impl_generics #schemars::JsonSchema for #ty #where_clause {
            #[inline]
            fn schema_name() -> #cow {
                ::std::borrow::Cow::Borrowed(::core::stringify!(#ident))
            }

            #[inline]
            fn schema_id() -> #cow {
                ::std::borrow::Cow::Borrowed(#schema_id)
            }

            fn json_schema(_: &mut #schemars::SchemaGenerator) -> #schemars::Schema {
                let #len = ::core::mem::size_of::<Self>();
                #schemars::json_schema!({
                    "type": "string",
                    #description
                    "minLength": #len,
                    "maxLength": #len,
                    "pattern": <Self as #eva::str::HasPattern>::regex_pat_fullmatch(),
                })
            }
        }
    }
}

fn pattern(target: &Target, fields: &syn::Fields, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let field_tys = fields.iter().map(|field| &field.ty);
    let buf = local("buf");

    quote! {
        #[automatically_derived]
        impl #impl_generics #eva::str::HasPattern for #ty #where_clause {
            #[inline]
            fn pat_into(#buf: &mut ::std::string::String) {
                #(<#field_tys as #eva::str::HasPattern>::pat_into(#buf);)*
            }
        }
    }
}

/// Sound because the where-clause makes every field `FixedUtf8` (alignment 1, initialized UTF-8)
/// and `reject_repr` keeps the injected `#[repr(C)]` as the only layout, so the fields are
/// contiguous UTF-8; the layout asserts only catch violations early.
fn fixed_utf8(target: &Target, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();

    quote! {
        #[automatically_derived]
        unsafe impl #impl_generics #eva::str::FixedUtf8 for #ty #where_clause {}
    }
}

fn as_str(target: &Target, layout: &Layout, eva: &syn::Path) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let check = layout.use_check();

    quote! {
        impl #impl_generics #ty #where_clause {
            /// The string this value stores.
            #[inline]
            pub const fn as_str(&self) -> &::core::primitive::str {
                #check
                #eva::str::reinterpret(self)
            }
        }
    }
}

/// Checks the total length, then parses each field from its slice of the input.
fn from_str(
    target: &Target,
    layout: &Layout,
    fields: &syn::Fields,
    error: &syn::Type,
    eva: &syn::Path,
) -> TokenStream {
    let (impl_generics, ty, where_clause) = target.split();
    let check = layout.use_check();
    let (s, rest, expected) = (local("s"), local("rest"), local("expected"));
    let values = fields.iter().map(|field| {
        let ty = &field.ty;
        quote! { #eva::str::parse_fixed_part::<#ty, #error>(&mut #rest)? }
    });
    let construct = construct(fields, values);

    quote! {
        #[automatically_derived]
        impl #impl_generics ::core::str::FromStr for #ty #where_clause {
            type Err = #error;

            #[inline]
            fn from_str(#s: &::core::primitive::str) -> ::core::result::Result<Self, #error> {
                #check
                let #expected = ::core::mem::size_of::<Self>();
                if #s.len() != #expected {
                    return ::core::result::Result::Err(
                        <#error as #eva::str::FixedParseError>::length(#expected),
                    );
                }
                let mut #rest = #s;
                ::core::result::Result::Ok(Self #construct)
            }
        }
    }
}

/// `path` reported at `span` but resolved as written, so `$crate` keeps working.
fn located_at(path: &syn::Path, span: Span) -> TokenStream {
    path.to_token_stream()
        .into_iter()
        .map(|mut token| {
            token.set_span(token.span().located_at(span));
            token
        })
        .collect()
}
