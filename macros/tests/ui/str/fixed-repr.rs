use eva::str::{Seq, ascii};

#[eva::str(fixed(error = eva::str::ParseError))]
#[repr(align(2))]
pub struct Aligned(Seq<2, ascii::Digit>);

fn main() {}
