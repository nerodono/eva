#[eva::data(error, display("text"))]
pub struct Text(#[source] String);

#[eva::data(error, display("generic"))]
pub enum Generic<E: std::fmt::Debug> {
    Inner(#[source] E),
}

fn main() {}
