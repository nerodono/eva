macro_rules! newtype {
    () => {
        #[eva::str(newtype, crate = $crate)]
        pub struct Name(String);
    };
}

newtype!();

fn main() {}
