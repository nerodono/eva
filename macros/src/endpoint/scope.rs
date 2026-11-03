use proc_macro2::TokenStream;
use quote::quote;

/// The service supplies the success lifetimes; `Output` is instantiated only by an endpoint,
/// in its local namespace. Keeping the helper on the method also supports re-exported methods.
pub(crate) fn export(method: &syn::Ident, lifetimes: &[syn::Lifetime]) -> TokenStream {
    let export = crate::service::unique("endpoint_scope", method);
    let generics = (!lifetimes.is_empty()).then(|| quote!(<#(#lifetimes),*>));
    quote! {
        #[doc(hidden)]
        pub use super::Result as __Result;

        #[doc(hidden)]
        #[macro_export]
        macro_rules! #export {
            ($scope:ident [$($method:tt)*]) => {
                #[allow(unused_imports)]
                mod __source {
                    // Resolve the caller's path separately: its imports must not collide
                    // with the method's `Args`, `Ok`, `Err`, or embedded items.
                    use super::*;
                    use $($method)* as __method;
                    pub mod $scope {
                        pub use super::__method::*;
                        #[allow(dead_code)]
                        pub type Output #generics = super::__method::__Result<Ok #generics, Err>;
                    }
                }
                #[allow(unused_imports)]
                use __source::$scope;
            };
        }
        #[doc(hidden)]
        pub use #export as __endpoint_scope;
    }
}

pub(super) fn declare(method: &syn::Path) -> TokenStream {
    // The resolver adds one module level. Bare paths use the caller's imports,
    // while crate-qualified and absolute paths already retain their meaning.
    let first = &method.segments[0].ident;
    let mut nested = method.clone();
    if first == "self" {
        nested.segments[0].ident = syn::Ident::new("super", first.span());
    } else if first == "super" {
        nested = syn::parse_quote!(super::#method);
    }
    quote! {
        #method::__endpoint_scope!(this [#nested]);
    }
}
