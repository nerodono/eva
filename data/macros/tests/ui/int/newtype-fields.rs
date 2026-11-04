#[eva::int(newtype)]
struct Two(u8, u8);

#[eva::int(newtype)]
struct Unit;

#[eva::int(newtype)]
struct Named {
    a: u8,
    b: u8,
}

fn main() {}
