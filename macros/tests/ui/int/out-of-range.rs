#[eva::int(u8, 300 | -1..=5 | 'я')]
pub enum Byte {}

#[eva::int(i8, b'\xff' | -129)]
pub enum Signed {}

#[eva::int(u128, 340282366920938463463374607431768211455)]
pub enum Huge {}

fn main() {}
