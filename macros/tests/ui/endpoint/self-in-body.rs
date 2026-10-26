#[eva::endpoint]
pub async fn UsesSelf(_state: (), _args: ()) {
    let _ = self;
}

fn main() {
    let _ = UsesSelf;
}
