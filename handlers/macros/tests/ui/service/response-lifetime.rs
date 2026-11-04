#[eva::service]
trait StructResponse {
    fn get<'a>() -> Struct! { value: &'a str };
}

#[eva::service]
trait EnumResponse {
    fn get<'a>() -> Enum! { Record { value: &'a str }, Tuple(&'a str), Simple };
}

fn main() {}
