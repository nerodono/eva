// Option and signature errors are reported together.
#[eva::endpoint(bogus)]
pub fn Noop(state: (), args: ()) {}

fn main() {
    let _ = Noop;
}
