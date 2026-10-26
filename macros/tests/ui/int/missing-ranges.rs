#[eva::int(u8)]
pub enum NoComma {}

#[eva::int(u8,)]
pub enum TrailingComma {}

#[eva::int(u8, 0 |)]
pub enum TrailingBar {}

#[eva::int]
pub enum NoArgs {}

fn main() {}
