use std::rc::Rc;

#[eva::endpoint]
pub async fn NotSend(_state: (), _args: ()) -> u32 {
    let rc = Rc::new(1);
    std::future::ready(()).await;
    *rc
}

fn main() {
    let _ = NotSend;
}
