// Gate generated integrations in the runtime crate: proc-macro features are unified
// separately from target dependencies and cannot reliably describe their features.
#[cfg(feature = "rand")]
#[doc(hidden)]
#[macro_export]
macro_rules! __with_rand {
    ($($tokens:tt)*) => { $($tokens)* };
}

#[cfg(not(feature = "rand"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __with_rand {
    ($($tokens:tt)*) => {};
}
