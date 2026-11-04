#[eva::int(u8, 0..=1, bogus)]
pub enum Bit {}

fn main() {
    let _: Option<Bit> = None;
}
