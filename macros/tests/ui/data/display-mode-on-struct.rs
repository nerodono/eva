/// Documented.
#[eva::data(display(doc))]
pub struct Doc;

#[eva::data(display(name))]
pub struct Name;

fn main() {
    let _ = (Doc, Name);
}
