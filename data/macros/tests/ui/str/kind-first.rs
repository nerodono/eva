#[eva::str(copy, newtype)]
pub struct Name(eva::str::CompactString);

fn main() {
    let _ = Name(Default::default());
}
