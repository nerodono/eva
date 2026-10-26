#[eva::int(u8, 0..=1)]
struct Bit(u8);

#[eva::int(u8, 0..=1)]
union Bits {
    a: u8,
}

fn main() {}
