use eva::str::{Seq, ascii};

#[eva::str(fixed)]
pub struct NoParens(Seq<2, ascii::Digit>);

#[eva::str(fixed(), copy)]
pub struct NoError(Seq<2, ascii::Digit>);

fn main() {}
