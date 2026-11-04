use eva::str::{ParseError, ascii};

#[eva::str(fixed(error = ParseError))]
pub struct Pin(ascii::Digit, u16);

fn main() {}
