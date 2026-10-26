#[eva::data]
#[display("struct")]
pub struct OnStruct {
    #[display("field")]
    field: u8,
}

#[eva::data(display(name))]
#[display("enum")]
pub enum OnEnum {
    A(#[display("variant field")] u8),
}

fn main() {
    let _ = (OnStruct { field: 1 }.field, OnEnum::A(1));
}
