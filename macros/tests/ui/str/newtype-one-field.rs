use eva::str::CompactString;

#[eva::str(newtype, not(schemars))]
pub struct Two(CompactString, u8);

#[eva::str(newtype, not(schemars))]
pub struct Unit;

#[eva::str(newtype, not(schemars))]
pub struct Empty {}

fn main() {}
