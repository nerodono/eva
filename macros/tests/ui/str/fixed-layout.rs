use std::str::FromStr;

use eva::str::{FixedUtf8, HasPattern, ParseError};

#[derive(Clone, Copy)]
#[repr(C, align(2))]
pub struct Wide([u8; 2]);

unsafe impl FixedUtf8 for Wide {}

impl HasPattern for Wide {
    fn pat_into(buf: &mut String) {
        buf.push_str("..");
    }
}

impl FromStr for Wide {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.as_bytes().try_into().map(Self).map_err(|_| ParseError::Length)
    }
}

#[eva::str(fixed(error = ParseError))]
pub struct Padded(Wide);

fn main() {}
