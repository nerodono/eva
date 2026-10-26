#[eva::data(error)]
pub enum Partial {
    #[display("a")]
    A,
    B(u8),
    C { x: u8 },
}

fn main() {
    let _ = (Partial::A, Partial::B(1), Partial::C { x: 1 });
}
