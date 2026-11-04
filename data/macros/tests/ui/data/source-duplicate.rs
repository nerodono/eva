#[eva::data(error, display("e"))]
pub struct Inner;

#[eva::data(error, display("outer"))]
pub enum Outer {
    Two(#[source] Inner, #[source] Inner),
}

fn main() {
    let _ = Outer::Two(Inner, Inner);
}
