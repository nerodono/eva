use eva::{export, service};

#[service]
pub trait Users {
    #[flags(wirte)]
    fn rename(name: String);

    #[flags(write, stream)]
    fn upload();
}

#[export(flags(write))]
pub struct Session {
    users: Users,
}

fn main() {}
