#[eva::endpoint]
pub async fn Variadic(state: (), args: (), rest: ...) {}

fn main() {
    let _ = Variadic;
}
