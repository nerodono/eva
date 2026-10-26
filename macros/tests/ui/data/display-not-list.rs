#[eva::data]
pub enum Forms {
    #[display]
    Path,
    #[display = "value"]
    NameValue,
}

fn main() {
    let _ = (Forms::Path, Forms::NameValue);
}
