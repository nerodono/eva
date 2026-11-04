use eva::service;

#[service]
pub trait Values {
    #[errors(Failed(First, Second))]
    fn get() -> ();
}

fn main() {}
