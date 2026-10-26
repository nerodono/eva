#[eva::int(u8, 0..=1, not(serde), not(schemars))]
pub enum Bit {}

fn main() {
    let _: Option<Bit> = None;
}
