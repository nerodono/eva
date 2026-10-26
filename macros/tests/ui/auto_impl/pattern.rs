#[eva::auto_impl(&)]
trait Pair {
    fn sum(&self, (a, b): (u32, u32)) -> u32 {
        a + b
    }
}

fn main() {}
