#[eva::data(display(doc))]
pub enum Computed {
    #[doc = concat!("computed", " docs")]
    A,
}

fn main() {
    let _ = Computed::A;
}
