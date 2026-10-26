#[eva::int(newtype)]
struct Wrapper<T>(T);

#[eva::int(newtype)]
struct Bounded(u8)
where
    u8: Copy;

fn main() {}
