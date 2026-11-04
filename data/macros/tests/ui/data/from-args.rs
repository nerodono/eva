#[eva::data]
pub enum Args {
    List(#[from(u8)] u8),
    Value(#[from = "u16"] u16),
}

fn main() {
    let _ = (Args::List(1), Args::Value(1));
}
