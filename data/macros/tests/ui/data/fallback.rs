// The fallback drops the attributes the skipped derives would register, so only the option
// error is reported.
#[eva::data(bogus)]
#[serde(tag = "kind")]
#[schemars(description = "wrapper")]
pub enum Wrapper {
    #[display("wrapped")]
    #[serde(rename = "w")]
    Wrapped(#[from] u8),
    Named {
        #[serde(default)]
        #[schemars(range(min = 1))]
        #[source]
        field: u8,
    },
}

fn main() {
    let _ = Wrapper::Wrapped(1);
    let _ = Wrapper::Named { field: 1 };
}
