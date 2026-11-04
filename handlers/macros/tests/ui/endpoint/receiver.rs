pub struct Service;

impl Service {
    #[eva::endpoint]
    pub async fn Receiver(&self, args: ()) {}
}

fn main() {}
