#[eva::data]
pub enum Variant {
    Two(#[from] u8, u16),
}

#[eva::data]
pub struct Struct {
    #[from]
    a: u8,
    b: u16,
}

fn main() {
    let s = Struct { a: 1, b: 2 };
    let _ = (Variant::Two(1, 2), s.a, s.b);
}
