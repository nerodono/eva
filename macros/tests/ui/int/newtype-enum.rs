#[eva::int(newtype)]
enum Kind {
    A,
}

#[eva::int(newtype)]
union Bits {
    a: u8,
}

fn main() {}
