use eva::int::InfallibleInteger;

#[eva::int(newtype(n > 1024))]
struct HighPort(std::num::NonZeroU16);

fn infallible<T: InfallibleInteger>() {}

fn main() {
    infallible::<HighPort>();
    let _ = HighPort::new(std::num::NonZeroU16::MIN);
}
