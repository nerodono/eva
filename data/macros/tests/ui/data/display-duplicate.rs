#[eva::data]
pub enum Twice {
    #[display("first")]
    #[display("second")]
    A,
}

fn main() {
    let _ = Twice::A;
}
