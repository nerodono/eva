use std::{
    num::NonZeroU64,
    sync::{Arc, atomic},
    time::{Duration, SystemTime},
};

use crate::time::Timestamp;

#[crate::auto_impl(&, &mut)]
pub trait Clock {
    fn get(&self) -> Timestamp;
}

#[derive(Debug, Clone)]
pub struct Mock(Arc<atomic::AtomicU64>);

impl Default for Mock {
    fn default() -> Self {
        Self(Arc::new(atomic::AtomicU64::new(
            Timestamp::TEST_ORIGIN.as_nanos().get(),
        )))
    }
}

impl Mock {
    pub fn advance(&mut self, dur: Duration) {
        self.0
            .fetch_add(dur.as_nanos() as u64, atomic::Ordering::Release);
    }

    pub fn set(&mut self, ts: Timestamp) {
        self.0.store(ts.as_nanos().get(), atomic::Ordering::Release);
    }

    pub fn back(&self, dur: Duration) {
        _ = dur;
        todo!()
    }
}

impl Clock for Mock {
    fn get(&self) -> Timestamp {
        let time = self.0.load(atomic::Ordering::Acquire);
        Timestamp::from_nanos(time.try_into().expect("invalid time set"))
    }
}

/// Real time clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealTime(());

impl Clock for RealTime {
    fn get(&self) -> Timestamp {
        // TODO: get time with a fixed time zone.
        let dur = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let nanos = NonZeroU64::new(dur.as_nanos() as u64).unwrap();

        Timestamp::from_nanos(nanos)
    }
}
