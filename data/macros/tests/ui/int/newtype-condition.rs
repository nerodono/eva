#[eva::int(newtype())]
struct Empty(u8);

#[eva::int(newtype(n > 1 extra))]
struct Leftover(u8);

#[eva::int(newtype(n > 1), bogus)]
struct Unknown(u8);

#[eva::int(newtype(n + 1))]
struct NotBool(u8);

fn main() {}
