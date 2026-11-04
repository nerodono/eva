#[eva::str(newtype, copy, copy)]
pub struct Name(eva::str::CompactString);

fn main() {
    let _ = Name(Default::default());
}
