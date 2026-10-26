/// The fn's attributes split by the generated item they apply to, each list in source order.
#[derive(Default)]
pub struct Routed<'a> {
    pub on_struct: Vec<&'a syn::Attribute>,
    pub on_impl: Vec<&'a syn::Attribute>,
    pub on_call: Vec<&'a syn::Attribute>,
    /// The struct is `#[deprecated]`, so the impl must not name it directly.
    pub deprecated: bool,
}

/// `#[cfg]` and `#[cfg_attr]` never reach here: rustc evaluates them before the macro runs.
pub fn route(attrs: &[syn::Attribute]) -> Routed<'_> {
    let mut routed = Routed::default();
    for attr in attrs {
        let name = attr.path().get_ident().map(ToString::to_string);
        match name.as_deref() {
            Some("doc") => routed.on_struct.push(attr),
            Some("deprecated") => {
                routed.on_struct.push(attr);
                routed.deprecated = true;
            }
            Some("allow" | "warn" | "deny" | "forbid") => {
                routed.on_struct.push(attr);
                routed.on_impl.push(attr);
            }
            // A copy on the struct would be reported as unfulfilled whenever the lint fires
            // only in the body.
            Some("expect") => routed.on_impl.push(attr),
            _ => routed.on_call.push(attr),
        }
    }
    routed
}

#[cfg(test)]
mod tests {
    use quote::{ToTokens as _, quote};
    use syn::parse::Parser as _;

    use super::*;

    fn paths(attrs: &[&syn::Attribute]) -> Vec<String> {
        let path = |attr: &&syn::Attribute| attr.path().to_token_stream().to_string();
        attrs.iter().map(path).collect()
    }

    #[test]
    fn routes_by_name_in_source_order() {
        let attrs = syn::Attribute::parse_outer
            .parse2(quote! {
                /// Docs.
                #[fastrace::trace]
                #[deprecated]
                #[allow(dead_code)]
                #[expect(unused)]
                #[inline]
                #[doc(alias = "x")]
            })
            .unwrap();
        let routed = route(&attrs);

        assert_eq!(
            paths(&routed.on_struct),
            ["doc", "deprecated", "allow", "doc"]
        );
        assert_eq!(paths(&routed.on_impl), ["allow", "expect"]);
        assert_eq!(paths(&routed.on_call), ["fastrace :: trace", "inline"]);
        assert!(routed.deprecated);
    }
}
