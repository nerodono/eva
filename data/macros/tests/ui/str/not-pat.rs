#[eva::str(fixed(error = eva::str::ParseError), not(pat))]
pub struct Fixed(eva::str::ascii::Digit);

#[eva::str(custom, not(pat))]
pub struct Custom(String);

fn main() {}
