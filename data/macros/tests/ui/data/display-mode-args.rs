#[eva::data(display(name, extra))]
pub enum Type {
    A,
}

#[eva::data]
pub enum Variant {
    #[display(doc, "extra", 1)]
    /// Docs.
    A,
}

fn main() {
    let _ = (Type::A, Variant::A);
}
