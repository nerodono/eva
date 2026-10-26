#[eva::str(newtype, not(Hash))]
pub struct Name(eva::str::CompactString);

fn main() {
    let _ = Name(Default::default());
}
