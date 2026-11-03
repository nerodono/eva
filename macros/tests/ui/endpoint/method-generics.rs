#[eva::endpoint(Greetings::greet<u32>)]
async fn Wrong(_state: (), _args: ()) {}

fn main() {
    let _ = Wrong;
}
