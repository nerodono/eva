#[eva::data(error, display("e"))]
pub struct Inner;

#[eva::data(error, display("outer"))]
#[source]
pub enum Outer {
    #[source]
    A(Inner),
}

fn main() {
    let _ = Outer::A(Inner);
}
