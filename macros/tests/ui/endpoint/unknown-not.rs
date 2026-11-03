#[eva::endpoint(not(tracing))]
async fn Wrong(_state: (), _args: ()) {}

fn main() {
    let _ = Wrong;
}
