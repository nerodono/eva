use crate::int;

pub fn push_ascii_pat(u: u8, to: &mut String) {
    let push = match u {
        b'\r' => r"\r",
        b'\n' => r"\n",
        b'-' => r"\-",
        b'^' => r"\^",
        b'$' => r"\$",
        b'[' => r"\[",
        b']' => r"\]",
        b'\\' => r"\\",
        b'.' => r"\.",
        b'*' => r"\*",
        b'+' => r"\+",
        b'?' => r"\?",
        b'{' => r"\{",
        b'}' => r"\}",
        b'|' => r"\|",
        b'(' => r"\(",
        b')' => r"\)",
        _ => {
            to.push(u as char);
            return;
        }
    };
    to.push_str(push);
}

#[macro_export]
macro_rules! push_ascii_pat {
    ($e:ident, $to:expr) => {{
        let buf = $to;
        let requires_brackets = match $e::RANGES.as_slice() {
            [r] => r.len() > 1,
            [] => false,
            _ => true,
        };
        if requires_brackets {
            buf.push('[');
        }
        for range in $e::RANGES.iter().cloned() {
            let start = *range.start();
            let end = *range.end();
            $crate::str::ascii::push_ascii_pat(start, buf);
            if start != end {
                buf.push('-');
                $crate::str::ascii::push_ascii_pat(end, buf);
            }
        }
        if requires_brackets {
            buf.push(']');
        }
    }};
}

macro_rules! valid {
    ($e:ident) => {
        $crate::single_ascii_char!($e);

        unsafe impl $crate::str::FixedUtf8 for $e {}

        #[allow(dead_code)]
        impl $e {
            pub const fn as_str(&self) -> &str {
                $crate::str::reinterpret(self)
            }
        }
    };
}

pub(crate) use valid;

#[int(u8, b'.', crate = crate)]
pub enum Dot {}
valid!(Dot);

#[int(u8, b':', crate = crate)]
pub enum Colon {}
valid!(Colon);

/// URL-safe character.
#[int(u8, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'@', crate = crate)]
pub enum UrlSafe {}
valid!(UrlSafe);

#[int(u8, b' ', crate = crate)]
pub enum Space {}
valid!(Space);

#[int(u8, b'0'..=b'9', crate = crate)]
pub enum Digit {}
valid!(Digit);

impl Digit {
    pub const fn parse(self) -> u8 {
        self as u8 - b'0'
    }
}

/// A valid ASCII character.
#[int(u8, 0..=127, crate = crate)]
pub enum Char {}
valid!(Char);

/// Printable ASCII character.
#[int(u8, 32..=126, crate = crate)]
pub enum Printable {}
valid!(Printable);

/// ASCII control character.
#[int(u8, 0..=31, crate = crate)]
pub enum Control {}
valid!(Control);
