#[eva::int(newtype(self.0 > 1))]
struct Port(u16);

#[eva::int(newtype, (n > 1))]
struct Comma(u16);

#[eva::int(crate = ::eva, newtype)]
struct Late(u16);

fn main() {}
