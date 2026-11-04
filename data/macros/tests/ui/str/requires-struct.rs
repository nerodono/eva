#[eva::str(fixed(error = eva::str::ParseError))]
pub enum Fixed {}

#[eva::str(newtype)]
pub union Newtype {
    inner: u8,
}

fn main() {}
