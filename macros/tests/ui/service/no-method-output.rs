#[eva::service]
pub trait Greetings {
    fn greet(name: String) -> String;
}

fn output(_: Greetings::greet::Output) {}

fn main() {}
