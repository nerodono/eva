#[eva::data(not(Debug, Hash))]
pub struct Unit;

fn main() {
    let _ = Unit;
}
