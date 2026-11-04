#[eva::data(display(doc))]
pub enum TypeDefault {
    /// Documented.
    A,
    B,
}

#[eva::data]
pub enum Own {
    #[display(doc)]
    A,
}

fn main() {
    let _ = (TypeDefault::A, TypeDefault::B, Own::A);
}
