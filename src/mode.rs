use std::sync::{
    atomic::{
        AtomicU32,
        Ordering::{AcqRel, Acquire},
    },
    Arc,
};
use tokio::sync::Notify;

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Mode {
    Once,
    Repeat,
    Stop,
}

#[derive(Clone)]
pub struct Subscriber(Arc<Internal>);
pub struct Listener(Arc<Internal>);

struct Internal {
    mode: AtomicU32,
    notify: Notify,
}

pub fn mode_manager(init: Mode) -> (Subscriber, Listener) {
    let internal = Arc::new(Internal {
        mode: AtomicU32::new(init as _),
        notify: Notify::new(),
    });
    (Subscriber(internal.clone()), Listener(internal))
}

impl Subscriber {
    #[inline]
    pub fn send(&self, mode: Mode) {
        if self.0.mode.swap(mode as _, AcqRel) != mode as _ {
            self.0.notify.notify_one();
        }
    }
}

impl Listener {
    #[inline]
    pub fn compair_exchange(&self, current: Mode, new: Mode) -> Mode {
        match self
            .0
            .mode
            .compare_exchange(current as _, new as _, AcqRel, Acquire)
        {
            Ok(_) => new,
            Err(err) => unsafe { std::mem::transmute(err) },
        }
    }

    #[inline]
    pub fn get(&self) -> Mode {
        let val = self.0.mode.load(Acquire);
        unsafe { std::mem::transmute(val) }
    }

    #[inline]
    pub async fn wait_for(&self, mut f: impl FnMut(Mode) -> bool) -> Mode {
        loop {
            let val = self.get();
            if f(val) {
                break val;
            }
            self.0.notify.notified().await;
        }
    }
}
