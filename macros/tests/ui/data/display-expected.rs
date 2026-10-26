#[eva::data(display())]
pub struct Empty;

#[eva::data(display(nmae))]
pub enum Typo {
    A,
}

#[eva::data]
pub enum NotALiteral {
    #[display(concat!("a", "b"))]
    A,
}

fn main() {
    let _ = (Empty, Typo::A, NotALiteral::A);
}
