#[eva::endpoint(crate = ::eva, crate = ::eva)]
pub async fn Noop(state: (), args: ()) {}

fn main() {
    let _ = Noop;
}
