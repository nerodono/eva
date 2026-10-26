#[eva::endpoint]
#[must_use = "the count"]
pub async fn Count(state: (), args: ()) -> u32 {
    0
}

fn main() {
    let _ = Count;
}
