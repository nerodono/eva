#![deny(deprecated)]

#[deprecated]
fn old_api() {}

#[eva::endpoint]
#[deprecated]
pub async fn Old(_state: (), _args: ()) {
    old_api();
}

fn main() {}
