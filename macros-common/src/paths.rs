use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;

/// Find one Cargo package in the caller, respecting dependency renames.
/// Candidate selection and paths inside the package belong to each macro crate.
pub fn resolve_crate(package: &str) -> Result<syn::Path, proc_macro_crate::Error> {
    let name = match crate_name(package)? {
        FoundCrate::Itself => package.replace('-', "_"),
        FoundCrate::Name(name) => name,
    };
    // Runtime crates alias themselves with their library name. An absolute path
    // also works in their integration tests, where `crate` refers to the test.
    let name = syn::Ident::new(&name, Span::call_site());
    Ok(syn::parse_quote!(::#name))
}

/// Resolve available packages in caller-supplied order. Keep the package name so
/// callers can select its reexport layout; use `.next()` when only one is needed.
pub fn resolve_crates<'a>(
    packages: &'a [&'a str],
) -> impl Iterator<Item = (&'a str, syn::Path)> + 'a {
    packages
        .iter()
        .filter_map(|&package| resolve_crate(package).ok().map(|path| (package, path)))
}
