#[eva::auto_impl(&, Rc)]
trait Counter {
    fn bump(&mut self);
}

fn main() {}
