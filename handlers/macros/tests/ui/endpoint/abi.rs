#[eva::endpoint]
pub async extern "C" fn Abi(state: (), args: ()) {}

fn main() {
    let _ = Abi;
}
