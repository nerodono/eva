use eva::service;

#[service]
pub trait Values {
    #[errors(first::Failed, Failed(second::Error))]
    fn get() -> ();
}

fn main() {}
