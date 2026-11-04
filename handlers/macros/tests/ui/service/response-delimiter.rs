#[eva::service]
trait StructResponse {
    fn get() -> Struct!(value: String);
}

#[eva::service]
trait EnumResponse {
    fn get() -> Enum![Simple];
}

fn main() {}
