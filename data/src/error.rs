use core::{error::Error, panic::UnwindSafe};

/// Views a `#[source]` field as `dyn Error`, including boxed trait objects, which don't
/// implement `Error` themselves; called through autoderef.
pub trait AsDynError<'a> {
    fn as_dyn_error(&self) -> &(dyn Error + 'a);
}

impl<'a, T: Error + 'a> AsDynError<'a> for T {
    #[inline]
    fn as_dyn_error(&self) -> &(dyn Error + 'a) {
        self
    }
}

macro_rules! impl_for_dyn {
    ($([$($bound:path),*])*) => {$(
        impl<'a> AsDynError<'a> for dyn Error $(+ $bound)* + 'a {
            #[inline]
            fn as_dyn_error(&self) -> &(dyn Error + 'a) {
                self
            }
        }
    )*};
}

impl_for_dyn!([] [Send] [Send, Sync] [Send, Sync, UnwindSafe]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxed_source() {
        let boxed: Box<dyn Error + Send + Sync> = "inner".into();
        assert_eq!(boxed.as_dyn_error().to_string(), "inner");
    }
}
