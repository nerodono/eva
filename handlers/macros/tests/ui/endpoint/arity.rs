#[eva::endpoint]
pub async fn Zero() {}

#[eva::endpoint]
pub async fn One(state: ()) {}

#[eva::endpoint]
pub async fn Three(state: (), args: (), extra: ()) {}

fn main() {
    let _ = (Zero, One, Three);
}
