//! # Uncategorized useful functionality

use eyre::Context as _;

use std::{ffi::OsStr, fmt, path::PathBuf};

use crate::data;

#[data(crate = crate, not(Debug), display("<secret>"))]
pub enum SecretString {
    Plaintext(String),
    File(PathBuf),
    Credential(String),
}

impl SecretString {
    pub fn credential(name: impl Into<String>) -> Self {
        Self::Credential(name.into())
    }

    pub fn read(&self) -> eyre::Result<String> {
        match self {
            Self::Plaintext(t) => Ok(t.clone()),
            Self::File(f) => std::fs::read_to_string(f).wrap_err("failed to read secret string"),
            Self::Credential(name) => {
                let directory = std::env::var_os("CREDENTIALS_DIRECTORY");
                let path = credential_path(name, directory.as_deref())?;
                std::fs::read_to_string(path)
                    .wrap_err_with(|| format!("failed to read credential {name:?}"))
            }
        }
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

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

fn credential_path(name: &str, directory: Option<&OsStr>) -> eyre::Result<PathBuf> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        eyre::bail!(
            "credential name must be a single file name containing letters, digits, '.', '_' or '-'"
        );
    }
    let directory = directory.ok_or_else(|| eyre::eyre!("CREDENTIALS_DIRECTORY is not set"))?;
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() {
        eyre::bail!("CREDENTIALS_DIRECTORY must be an absolute path");
    }
    Ok(directory.join(name))
}

#[cfg(test)]
mod secret_tests {
    use super::*;

    #[test]
    fn credential_names_stay_inside_the_directory() {
        let dir = Some(OsStr::new("/run/credentials/example.service"));
        assert_eq!(
            credential_path("weather.api-key", dir).unwrap(),
            PathBuf::from("/run/credentials/example.service/weather.api-key")
        );
        for name in [
            "", ".", "..", "../key", "/key", "a/b", "a\\b", "a:../b", "a\0b",
        ] {
            assert!(credential_path(name, dir).is_err(), "{name:?}");
        }
        assert!(credential_path("key", None).is_err());
        assert!(credential_path("key", Some(OsStr::new("relative"))).is_err());
    }

    #[test]
    fn credentials_are_tagged_and_redacted() {
        let secret: SecretString =
            serde_json::from_str(r#"{"credential":"weather-api-key"}"#).unwrap();
        assert_eq!(secret, SecretString::credential("weather-api-key"));
        assert_eq!(format!("{secret}"), "<secret>");
        assert_eq!(format!("{secret:?}"), "<secret>");
        assert_eq!(
            serde_json::to_string(&secret).unwrap(),
            r#"{"credential":"weather-api-key"}"#
        );
    }
}
