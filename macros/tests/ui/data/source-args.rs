#[eva::data(error, display("e"))]
pub struct Inner;

#[eva::data(error, display("outer"))]
pub struct Outer(#[source(inner)] Inner);

fn main() {
    let _ = Outer(Inner);
}
