#[eva::data(not(as_static_str))]
pub struct Unit;

fn main() {
    let _ = Unit;
}
