macro_rules! define {
    ($name:ident) => {
        #[eva::data(crate = $crate, not(serde))]
        pub struct $name;
    };
}

define!(Unit);

fn main() {
    let _ = Unit;
}
