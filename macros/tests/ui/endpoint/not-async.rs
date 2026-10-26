#[eva::endpoint]
pub fn Blocking(state: (), args: ()) {}

fn main() {
    let _ = Blocking;
}
