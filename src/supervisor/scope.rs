use std::{
    cell::RefCell,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    thread_local,
};

use pin_project_lite::pin_project;

thread_local! {
    static SCOPE: RefCell<Option<Arc<Scope>>> = const { RefCell::new(None) };
}

pin_project! {
    pub struct FutureInScope<F> {
        scope: Arc<Scope>,
        #[pin]
        fut: F,
    }
}

impl<F: Future> Future for FutureInScope<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        SCOPE.with_borrow_mut(|opt| match opt {
            Some(arc) => {
                if !Arc::ptr_eq(arc, &*this.scope) {
                    *arc = Arc::clone(&*this.scope);
                }
            }
            None => *opt = Some(Arc::clone(this.scope)),
        });

        this.fut.poll(cx)
    }
}

pub struct Scope {
    component: String,
}

impl Scope {
    pub fn new(component: &str) -> Self {
        Self {
            component: component.into(),
        }
    }

    pub fn component(&self) -> &str {
        self.component.as_str()
    }
}

pub trait InScopeExt {
    type InScopeFuture: Future;

    fn in_scope(self, scope: Arc<Scope>) -> Self::InScopeFuture;
}

impl<Fut: Future> InScopeExt for Fut {
    type InScopeFuture = FutureInScope<Fut>;

    fn in_scope(self, scope: Arc<Scope>) -> Self::InScopeFuture {
        FutureInScope { scope, fut: self }
    }
}

pub fn with<O>(f: impl FnOnce(&Scope) -> O) -> Option<O> {
    SCOPE.with_borrow(|scope| scope.as_deref().map(f))
}
