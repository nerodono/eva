#[eva::endpoint]
pub async unsafe fn Unsafe(state: (), args: ()) {}

fn main() {
    let _ = Unsafe;
}
