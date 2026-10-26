use std::{rc::Rc, sync::Arc};

#[eva::auto_impl(&, &mut, Box, Rc, Arc)]
trait Read<'a, K: ?Sized> {
    const ID: u8;
    type Item<'b>: AsRef<[u8]>
    where
        Self: 'b,
        K: 'a;

    fn get<'b>(&'b self, key: &'a K) -> Self::Item<'b>;
    fn size_of<T>() -> usize {
        std::mem::size_of::<T>()
    }
    fn count(&self) -> usize
    where
        Self: Clone;
}

#[derive(Clone)]
struct Store(Vec<u8>);

impl<'a> Read<'a, str> for Store {
    const ID: u8 = 3;
    type Item<'b> = &'b [u8];

    fn get<'b>(&'b self, key: &'a str) -> &'b [u8] {
        &self.0[..key.len()]
    }
    fn count(&self) -> usize {
        self.0.len()
    }
}

fn read<'a, R: Read<'a, str> + Clone>(r: R, key: &'a str) -> (u8, usize, usize, usize) {
    (
        R::ID,
        r.get(key).as_ref().len(),
        r.count(),
        R::size_of::<u32>(),
    )
}

#[test]
fn forwards_items_through_shared_proxies() {
    let store = Store(vec![1, 2, 3, 4]);
    let expected = (3, 2, 4, 4);
    assert_eq!(read(&store, "ab"), expected);
    assert_eq!(read(Rc::new(store.clone()), "ab"), expected);
    assert_eq!(read(Arc::new(store.clone()), "ab"), expected);
    assert_eq!(read(Box::new(store), "ab"), expected);
}

#[eva::auto_impl(&mut, Box)]
trait Sink: Send {
    fn push(&mut self, value: u32);
    fn push_twice(&mut self, mut value: u32) {
        self.push(value);
        value += 1;
        self.push(value);
    }
}

#[eva::auto_impl(Box)]
trait Consume {
    fn take(self) -> u32;
}

impl Consume for u32 {
    fn take(self) -> u32 {
        self
    }
}

impl Consume for Vec<u32> {
    fn take(self) -> u32 {
        self.into_iter().sum()
    }
}

impl Sink for Vec<u32> {
    fn push(&mut self, value: u32) {
        Vec::push(self, value);
    }
}

#[test]
fn forwards_mutable_and_owned_receivers() {
    let mut sink = Vec::new();
    <&mut Vec<u32> as Sink>::push(&mut &mut sink, 1);
    let mut boxed: Box<Vec<u32>> = Box::new(sink);
    boxed.push_twice(2);
    assert_eq!(*boxed, [1, 2, 3]);
    assert_eq!(Box::new(*boxed).take(), 6);
    assert_eq!(Box::new(5_u32).take(), 5);
}

#[eva::auto_impl(&, Arc)]
trait Fetch: Sync {
    async fn fetch(&self, id: u32) -> u32;
}

struct Double;
impl Fetch for Double {
    async fn fetch(&self, id: u32) -> u32 {
        id * 2
    }
}

#[eva::auto_impl(&)]
trait Named {
    fn name(&self) -> &'static str;
}

impl Named for str {
    fn name(&self) -> &'static str {
        "str"
    }
}

#[test]
fn forwards_async_and_unsized() {
    let fetcher = Arc::new(Double);
    let fut = <Arc<Double> as Fetch>::fetch(&fetcher, 21);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut fut = std::pin::pin!(fut);
    assert_eq!(fut.as_mut().poll(&mut cx), std::task::Poll::Ready(42));
    let s: &str = "x";
    assert_eq!(<&str as Named>::name(&s), "str");
}
