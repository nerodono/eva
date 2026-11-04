#[eva::int(i8, -b'a')]
pub enum Byte {}

#[eva::int(i32, 0 | -'a'..='z')]
pub enum Char {}

fn main() {}
