use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
    pin::PinMut,
    time::{Duration, SystemTime},
};
use execution_context::ExecutionContext;
use futures::{Future, Poll, task};

pub mod deadline_compat;
#[cfg(feature = "serde")]
pub mod serde;
pub mod stream;

/// Types that can be represented by a [`Duration`].
pub trait AsDuration {
    fn as_duration(&self) -> Duration;
}

impl AsDuration for SystemTime {
    /// Duration of 0 if self is earlier than [`SystemTime::now`].
    fn as_duration(&self) -> Duration {
        self.duration_since(SystemTime::now()).unwrap_or_default()
    }
}

/// Collection compaction; configurable `shrink_to_fit`.
pub trait Compact {
    /// Compacts space if the ratio of length : capacity is less than `usage_ratio_threshold`.
    fn compact(&mut self, usage_ratio_threshold: f64);
}

impl<K, V, H> Compact for HashMap<K, V, H>
where
    K: Eq + Hash,
    H: BuildHasher,
{
    fn compact(&mut self, usage_ratio_threshold: f64) {
        let usage_ratio = self.len() as f64 / self.capacity() as f64;
        if usage_ratio < usage_ratio_threshold {
            self.shrink_to_fit();
        }
    }
}

/// Returns a future that executes within the scope of the current [ExecutionContext].
pub fn context_propagating<F: Future>(future: F) -> impl Future<Output = F::Output> {
    ContextFuture {
        future,
        context: ExecutionContext::capture(),
    }
}

/// A future that executes within a specific [ExecutionContext].
struct ContextFuture<F> {
    future: F,
    context: ExecutionContext,
}

impl<F> Future for ContextFuture<F>
    where
        F: Future,
{
    type Output = F::Output;

    fn poll(self: PinMut<Self>, cx: &mut task::Context) -> Poll<F::Output> {
        let me = unsafe { PinMut::get_mut_unchecked(self) };
        let future = unsafe { PinMut::new_unchecked(&mut me.future) };
        me.context.run(|| future.poll(cx))
    }
}

#[test]
fn propagate() {
    use crate::context::{self, Context};
    let ctx = Context {
        deadline: SystemTime::UNIX_EPOCH + Duration::from_secs(5),
        trace_context: trace::Context::new_root(),
    };
    context::set(ctx);
    let propagate = context_propagating(async {
        context::current()
    });
    let no_propagate = async {
        context::current()
    };
    let new_ctx = Context {
        deadline: SystemTime::UNIX_EPOCH,
        trace_context: trace::Context::new_root(),
    };
    context::set(new_ctx);

    let ctx2 = futures::executor::block_on(propagate);
    assert!(ctx.deadline == ctx2.deadline);
    assert!(ctx.trace_context == ctx2.trace_context);

    let ctx2 = futures::executor::block_on(no_propagate);
    assert!(new_ctx.deadline == ctx2.deadline);
    assert!(new_ctx.trace_context == ctx2.trace_context);
}
