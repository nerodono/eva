#[eva::endpoint]
pub const async fn Const(state: (), args: ()) {}

fn main() {
    let _ = Const;
}
