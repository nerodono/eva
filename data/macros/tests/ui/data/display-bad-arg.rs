#[eva::data(display("{}", 1 +))]
pub struct Unit;

#[eva::data]
pub enum Enum {
    #[display("{}" _0)]
    A(u8),
}

fn main() {
    let _ = (Unit, Enum::A(1));
}
