//! # Uncategorized useful functionality

/// Hint that this branch is cold.
#[inline(always)]
pub const fn cold<T>(v: T) -> T {
    v
}

/// Indicate that this value is unlikely to be true. Hint to
/// the compiler.
#[inline(always)]
pub const fn unlikely(v: bool) -> bool {
    if v { cold(true) } else { false }
}

/// Indicate that this value is likely to be true. Hint to
/// the compiler.
#[inline(always)]
pub const fn likely(v: bool) -> bool {
    if v { true } else { cold(false) }
}
