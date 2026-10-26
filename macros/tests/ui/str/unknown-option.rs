#[eva::str(newtype, bogus)]
pub struct Name(eva::str::CompactString);

fn main() {
    let _ = Name(Default::default());
}
