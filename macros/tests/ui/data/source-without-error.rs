#[eva::data(error, display("e"))]
pub struct Inner;

#[eva::data(display("outer"))]
pub struct Outer(#[source] Inner);

fn main() {
    let _ = Outer(Inner);
}
