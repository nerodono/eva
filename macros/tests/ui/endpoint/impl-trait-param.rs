#[eva::endpoint]
pub async fn ImplParam(state: (), args: Vec<impl Send>) {}

fn main() {
    let _ = ImplParam;
}
