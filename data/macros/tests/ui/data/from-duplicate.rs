#[eva::data]
pub enum Twice {
    A(#[from] #[from] u8),
}

fn main() {
    let _ = Twice::A(1);
}
