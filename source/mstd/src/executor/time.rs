use heapless::Vec;

use crate::utils::ArfCell;
use core::{ops::{Sub, Add, DerefMut}, future::Future, pin::Pin, task::{Context, Poll, Waker}};
pub use core::time::Duration;


// TODO: This is an `ArfCell` and not just a plain atomic in order to
// support 32-bit targets. It might be worth specializing this to make it
// more efficient on 64-bit targets
pub static CURRENT_TIME: ArfCell<u64> = ArfCell::new(0);
const TICKS_PER_SEC: u64 = 1_000_000;
pub(crate) static CHRONOS: Chronos = Chronos {
    inner: ArfCell::new(ChronosInner {
        shorts: Vec::new(),
    }),
};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Alarm {
    tick: u64
}

impl Alarm {
    pub fn after(dur: Duration) -> Self {
        Self {
            tick: Instant::now().add(dur).tick,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now().tick >= self.tick
    }

    pub const fn never() -> Self {
        Self {
            tick: u64::MAX,
        }
    }
}

impl Future for Alarm {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.is_expired() {
            Poll::Ready(())
        } else {
            CHRONOS.register(&self, cx.waker());
            Poll::Pending
        }
    }
}

#[derive(Copy, Clone)]
pub struct Instant {
    tick: u64,
}

impl Instant {
    pub fn now() -> Self {
        Self {
            tick: *CURRENT_TIME.borrow().unwrap(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        let now = Instant::now();
        now - *self
    }
}

impl Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        let delta = self.tick.checked_sub(rhs.tick).unwrap();
        Duration::from_micros(delta)
    }
}

impl Add<Duration> for Instant {
    type Output = Instant;

    fn add(mut self, other: Duration) -> Self::Output {
        // Do this to avoid u128s
        let dur = other.as_secs() * TICKS_PER_SEC;
        let dur = dur + other.subsec_micros() as u64;
        self.tick += dur;
        self
    }
}

impl PartialEq for Alarmed {
    fn eq(&self, other: &Self) -> bool {
        self.alarm.eq(&other.alarm)
    }
}

impl Eq for Alarmed {}

impl PartialOrd for Alarmed {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.alarm.partial_cmp(&other.alarm)
    }
}

impl Ord for Alarmed {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.alarm.cmp(&other.alarm)
    }
}

struct Alarmed {
    waker: Waker,
    alarm: Alarm,
}

struct ChronosInner {
    shorts: Vec<Alarmed, 32>,
}

pub(crate) struct Chronos {
    inner: ArfCell<ChronosInner>,
}

impl Chronos {
    pub(crate) fn poll(&self) {
        let curr_tick = *CURRENT_TIME.borrow().unwrap();

        let mut inner = self.inner.borrow_mut().unwrap();
        if let Some(alm) = inner.shorts.get(0) {
            if alm.alarm.tick > curr_tick {
                // Nothing to wake, we're done
                return;
            }
        } else {
            // No items, we're done
            return;
        }

        let mut new = Vec::new();
        core::mem::swap(&mut new, &mut inner.shorts);
        for almd in new.into_iter() {
            if almd.alarm.tick <= curr_tick {
                almd.waker.wake();
            } else {
                inner.shorts.push(almd).ok();
            }
        }
    }

    fn register(&self, alarm: &Alarm, waker: &Waker) {
        let mut inner = self.inner.borrow_mut().unwrap();
        let almd = Alarmed {
            alarm: alarm.clone(),
            waker: waker.clone(),
        };

        let _almd = match inner.shorts.push(almd) {
            Ok(()) => {
                inner.shorts.deref_mut().sort_unstable();
                return;
            },
            Err(almd) => almd,
        };

        panic!("Timer overflow!");

        // The list is full. See if the new item is sooner than any on the list
        // let worked = if let Some(last) = inner.shorts.last_mut() {
        //     if last < &mut almd {
        //         core::mem::swap(last, &mut almd);
        //         true
        //     } else {
        //         false
        //     }
        // } else {
        //     false
        // };

        // We now need to put whatever is in `almd` into the purgatory queue
        // self.purgatory.enqueue(todo!("HOW DO I A GET A `HEADER`???"));
    }
}

