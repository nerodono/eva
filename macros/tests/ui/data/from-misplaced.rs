#[eva::data]
#[from]
pub struct OnItem(u8);

#[eva::data]
pub enum OnVariant {
    #[from]
    A(u8),
}

fn main() {
    let _ = (OnItem(1), OnVariant::A(1));
}
