use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    backend,
    model::{Action, ClickPosition},
};

pub struct ClickEngine {
    active: AtomicBool,
    interval_ms: AtomicU64,
    duration_ms: AtomicU64,
    remaining_ms: AtomicU64,
    max_actions: AtomicU64,
    remaining_actions: AtomicU64,
    completed_run: AtomicBool,
    action: RwLock<Action>,
    position: RwLock<Option<ClickPosition>>,
    backend_error: RwLock<Option<String>>,
}

impl ClickEngine {
    pub fn start() -> Arc<Self> {
        let engine = Arc::new(Self {
            active: AtomicBool::new(false),
            interval_ms: AtomicU64::new(100),
            duration_ms: AtomicU64::new(0),
            remaining_ms: AtomicU64::new(0),
            max_actions: AtomicU64::new(0),
            remaining_actions: AtomicU64::new(0),
            completed_run: AtomicBool::new(false),
            action: RwLock::new(Action::LeftClick),
            position: RwLock::new(None),
            backend_error: RwLock::new(None),
        });

        let worker_engine = Arc::clone(&engine);
        thread::spawn(move || worker_engine.worker());
        engine
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub fn set_active(&self, active: bool) {
        if active {
            self.completed_run.store(false, Ordering::Release);
        }
        self.active.store(active, Ordering::Release);
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn toggle(&self) {
        let was_active = self.active.fetch_xor(true, Ordering::AcqRel);
        if !was_active {
            self.completed_run.store(false, Ordering::Release);
        }
    }

    pub fn set_interval_ms(&self, interval_ms: u64) {
        self.interval_ms
            .store(interval_ms.max(10), Ordering::Release);
    }

    pub fn set_duration_ms(&self, duration_ms: u64) {
        self.duration_ms.store(duration_ms, Ordering::Release);
    }

    pub fn interval_ms(&self) -> u64 {
        self.interval_ms.load(Ordering::Acquire)
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms.load(Ordering::Acquire)
    }

    pub fn set_max_actions(&self, max_actions: u64) {
        self.max_actions.store(max_actions, Ordering::Release);
    }

    pub fn max_actions(&self) -> u64 {
        self.max_actions.load(Ordering::Acquire)
    }

    pub fn remaining_actions(&self) -> Option<u64> {
        let remaining = self.remaining_actions.load(Ordering::Acquire);
        (remaining > 0).then_some(remaining)
    }

    pub fn remaining_ms(&self) -> Option<u64> {
        let remaining = self.remaining_ms.load(Ordering::Acquire);
        (remaining > 0).then_some(remaining)
    }

    pub fn take_completed_run(&self) -> bool {
        self.completed_run.swap(false, Ordering::AcqRel)
    }

    pub fn set_action(&self, action: Action) {
        *self.action.write().expect("action lock poisoned") = action;
    }

    pub fn action(&self) -> Action {
        *self.action.read().expect("action lock poisoned")
    }

    pub fn set_position(&self, position: Option<ClickPosition>) {
        *self.position.write().expect("position lock poisoned") = position;
    }

    pub fn position(&self) -> Option<ClickPosition> {
        *self.position.read().expect("position lock poisoned")
    }

    pub fn backend_error(&self) -> Option<String> {
        self.backend_error
            .read()
            .expect("error lock poisoned")
            .clone()
    }

    fn worker(&self) {
        let mut backend = match backend::create() {
            Ok(backend) => backend,
            Err(error) => {
                *self.backend_error.write().expect("error lock poisoned") = Some(error);
                return;
            }
        };

        let mut was_active = false;
        let mut stop_at = None;
        let mut action_limit = 0;
        let mut actions_completed = 0;

        loop {
            if !self.is_active() {
                was_active = false;
                stop_at = None;
                self.remaining_ms.store(0, Ordering::Release);
                self.remaining_actions.store(0, Ordering::Release);
                thread::sleep(Duration::from_millis(20));
                continue;
            }

            // Let the GTK main loop hide the Start button before a synthetic
            // mouse click is delivered at the current pointer position.
            if !was_active {
                was_active = true;
                thread::sleep(Duration::from_millis(150));
                if !self.is_active() {
                    continue;
                }
                let duration_ms = self.duration_ms.load(Ordering::Acquire);
                stop_at =
                    (duration_ms > 0).then(|| Instant::now() + Duration::from_millis(duration_ms));
                self.remaining_ms.store(duration_ms, Ordering::Release);
                action_limit = self.max_actions.load(Ordering::Acquire);
                actions_completed = 0;
                self.remaining_actions
                    .store(action_limit, Ordering::Release);
            }

            if let Some(deadline) = stop_at {
                let now = Instant::now();
                if now >= deadline {
                    self.remaining_ms.store(0, Ordering::Release);
                    self.active.store(false, Ordering::Release);
                    self.completed_run.store(true, Ordering::Release);
                    continue;
                }
                self.remaining_ms.store(
                    deadline.duration_since(now).as_millis() as u64,
                    Ordering::Release,
                );
            }

            let started = Instant::now();
            let action = *self.action.read().expect("action lock poisoned");
            let position = *self.position.read().expect("position lock poisoned");
            if let Err(error) = backend.perform(action, position) {
                *self.backend_error.write().expect("error lock poisoned") = Some(error);
                self.set_active(false);
            } else if action_limit > 0 {
                actions_completed += 1;
                let remaining = action_limit.saturating_sub(actions_completed);
                self.remaining_actions.store(remaining, Ordering::Release);
                if remaining == 0 {
                    self.active.store(false, Ordering::Release);
                    self.completed_run.store(true, Ordering::Release);
                    continue;
                }
            }

            let interval = Duration::from_millis(self.interval_ms.load(Ordering::Acquire));
            thread::sleep(interval.saturating_sub(started.elapsed()));
        }
    }
}
