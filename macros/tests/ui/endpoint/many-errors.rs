#[eva::endpoint]
pub const unsafe extern "C" fn Many<T>(state: impl Send) -> impl Send {}

fn main() {
    let _ = Many;
}
