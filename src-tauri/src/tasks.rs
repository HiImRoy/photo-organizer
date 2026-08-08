use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Condvar, Mutex};

use crate::source_identity::is_same_or_descendant;

#[derive(Debug, Default)]
pub struct TaskRegistry {
    cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Debug, Default)]
pub struct SourceScanRegistry {
    active: Mutex<Vec<String>>,
}

#[derive(Debug)]
pub struct SourceScanGuard {
    registry: Arc<SourceScanRegistry>,
    identity_key: String,
}

impl SourceScanRegistry {
    pub fn try_acquire(self: &Arc<Self>, identity_key: &str) -> Option<SourceScanGuard> {
        let mut active = self.active.lock();
        if active.iter().any(|current| {
            is_same_or_descendant(current, identity_key)
                || is_same_or_descendant(identity_key, current)
        }) {
            return None;
        }
        active.push(identity_key.to_owned());
        Some(SourceScanGuard {
            registry: self.clone(),
            identity_key: identity_key.to_owned(),
        })
    }
}

impl Drop for SourceScanGuard {
    fn drop(&mut self) {
        self.registry
            .active
            .lock()
            .retain(|current| current != &self.identity_key);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticControlSignal {
    Continue,
    Cancel,
}

#[derive(Debug, Default)]
struct SemanticControlState {
    paused: bool,
    cancelled: bool,
}

#[derive(Debug, Default)]
pub struct SemanticTaskControl {
    state: Mutex<SemanticControlState>,
    wake: Condvar,
}

impl SemanticTaskControl {
    pub fn wait_until_runnable(&self) -> SemanticControlSignal {
        let mut state = self.state.lock();
        while state.paused && !state.cancelled {
            self.wake.wait(&mut state);
        }
        if state.cancelled {
            SemanticControlSignal::Cancel
        } else {
            SemanticControlSignal::Continue
        }
    }

    fn pause(&self) {
        self.state.lock().paused = true;
    }

    fn resume(&self) {
        let mut state = self.state.lock();
        state.paused = false;
        self.wake.notify_all();
    }

    fn cancel(&self) {
        let mut state = self.state.lock();
        state.cancelled = true;
        state.paused = false;
        self.wake.notify_all();
    }
}

#[derive(Debug, Default)]
pub struct SemanticTaskRegistry {
    controls: Mutex<HashMap<String, Arc<SemanticTaskControl>>>,
}

impl SemanticTaskRegistry {
    pub fn insert(&self, job_id: &str) -> Option<Arc<SemanticTaskControl>> {
        let mut controls = self.controls.lock();
        if controls.contains_key(job_id) {
            return None;
        }
        let control = Arc::new(SemanticTaskControl::default());
        controls.insert(job_id.to_owned(), control.clone());
        Some(control)
    }

    pub fn pause(&self, job_id: &str) -> bool {
        self.controls
            .lock()
            .get(job_id)
            .cloned()
            .is_some_and(|control| {
                control.pause();
                true
            })
    }

    pub fn resume(&self, job_id: &str) -> bool {
        self.controls
            .lock()
            .get(job_id)
            .cloned()
            .is_some_and(|control| {
                control.resume();
                true
            })
    }

    pub fn cancel(&self, job_id: &str) -> bool {
        self.controls
            .lock()
            .get(job_id)
            .cloned()
            .is_some_and(|control| {
                control.cancel();
                true
            })
    }

    pub fn remove(&self, job_id: &str) {
        self.controls.lock().remove(job_id);
    }
}

impl TaskRegistry {
    pub fn create(&self) -> (String, Arc<AtomicBool>) {
        let task_id = uuid::Uuid::new_v4().to_string();
        let token = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .insert(task_id.clone(), token.clone());
        (task_id, token)
    }

    pub fn cancel(&self, task_id: &str) -> bool {
        let token = self.cancellations.lock().get(task_id).cloned();
        if let Some(token) = token {
            token.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, task_id: &str) {
        self.cancellations.lock().remove(task_id);
    }

    #[cfg(test)]
    fn contains(&self, task_id: &str) -> bool {
        self.cancellations.lock().contains_key(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_explicit_and_task_scoped() {
        let registry = TaskRegistry::default();
        let (task_id, token) = registry.create();
        assert!(registry.contains(&task_id));
        assert!(!token.load(Ordering::Relaxed));
        assert!(registry.cancel(&task_id));
        assert!(token.load(Ordering::Relaxed));
        registry.remove(&task_id);
        assert!(!registry.cancel(&task_id));
    }

    #[test]
    fn overlapping_source_scans_are_serialized() {
        let registry = Arc::new(SourceScanRegistry::default());
        let parent = registry.try_acquire("c:/photos").expect("parent lock");
        assert!(registry.try_acquire("c:/photos/child").is_none());
        assert!(registry.try_acquire("c:/photos-other").is_some());
        drop(parent);
        assert!(registry.try_acquire("c:/photos/child").is_some());
    }

    #[test]
    fn semantic_task_controls_are_scoped_and_resumable() {
        let registry = SemanticTaskRegistry::default();
        let control = registry.insert("semantic-one").expect("new control");
        assert!(registry.pause("semantic-one"));
        assert!(registry.resume("semantic-one"));
        assert_eq!(
            control.wait_until_runnable(),
            SemanticControlSignal::Continue
        );
        assert!(registry.cancel("semantic-one"));
        assert_eq!(control.wait_until_runnable(), SemanticControlSignal::Cancel);
        registry.remove("semantic-one");
        assert!(!registry.cancel("semantic-one"));
    }
}
