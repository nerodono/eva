#[eva::endpoint]
pub async fn ReturnOnly<T: Default>(state: (), args: ()) -> T {
    T::default()
}

#[eva::endpoint]
pub async fn UnusedLifetime<'a>(state: (), args: ()) {}

#[eva::endpoint]
pub async fn BodyOnly<const N: usize>(state: (), args: ()) -> usize {
    N
}

#[eva::endpoint]
pub async fn ProjectionOnly<T: std::str::FromStr>(state: (), args: T::Err) {}

mod m {
    pub struct T;
}

#[eva::endpoint]
pub async fn ModulePath<T>(state: (), args: m::T) {}

#[eva::endpoint]
pub async fn QualifiedSelf<T>(state: (), args: <Vec<T> as IntoIterator>::IntoIter) {}

#[eva::endpoint]
pub async fn WhereOnly<T>(state: (), args: ())
where
    T: Send,
{
}

fn main() {
    let _ = (
        ReturnOnly,
        UnusedLifetime,
        BodyOnly,
        ProjectionOnly,
        ModulePath,
        QualifiedSelf,
        WhereOnly,
    );
}
