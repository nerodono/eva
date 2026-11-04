#[eva::endpoint]
pub async fn ImplReturn(state: (), args: ()) -> impl Send {}

fn main() {
    let _ = ImplReturn;
}
