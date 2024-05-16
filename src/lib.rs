use std::mem::ManuallyDrop;

type Id = u64;

pub struct TargetedBulk<E> {
    targets: Vec<u64>,
    events: Vec<ManuallyDrop<E>>,
}

impl<E> Default for TargetedBulk<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> TargetedBulk<E> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            targets: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, target: Id, data: E) {
        self.targets.push(target);
        self.events.push(ManuallyDrop::new(data));
    }

    pub fn drain_par<F>(&mut self, process: F)
    where
        F: Fn(Id, E) + Send + Sync,
        E: Send + Sync,
    {
        rayon::broadcast(|ctx| {
            let thread_idx = ctx.index() as u64;
            let num_threads = ctx.num_threads() as u64;

            for i in 0..self.events.len() {
                let target = self.targets[i];
                if target % num_threads != thread_idx {
                    continue;
                }

                let elem = unsafe { core::ptr::read(self.events.get_unchecked(i)) };
                process(target, ManuallyDrop::into_inner(elem));
            }
        });

        self.events.clear();
        self.targets.clear();
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn test_push_and_drain() {
        let mut bulk = TargetedBulk::new();
        bulk.push(1, "event1");
        bulk.push(2, "event2");

        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);

        bulk.drain_par(|target, data| {
            let mut results = results_clone.lock().unwrap();
            results.push((target, data));
        });

        let results = results.lock().unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains(&(1, "event1")));
        assert!(results.contains(&(2, "event2")));
    }

    #[test]
    fn test_drain_clears_events() {
        let mut bulk = TargetedBulk::new();
        bulk.push(1, "event1");
        bulk.push(2, "event2");

        bulk.drain_par(|_, _| {});

        assert!(bulk.targets.is_empty());
        assert!(bulk.events.is_empty());
    }

    #[test]
    fn test_concurrent_processing() {
        let mut bulk = TargetedBulk::new();
        for i in 0..100 {
            bulk.push(i, i * 2);
        }

        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);

        bulk.drain_par(|target, data| {
            let mut results = results_clone.lock().unwrap();
            results.push((target, data));
        });

        let results = results.lock().unwrap();
        assert_eq!(results.len(), 100);
        for i in 0..100 {
            assert!(results.contains(&(i, i * 2)));
        }
    }

    #[test]
    fn test_random_push_and_drain() {
        fastrand::seed(42);
        for i in 0..100 {
            let data: Vec<_> = (0..i)
                .map(|_| (fastrand::u64(..), fastrand::u64(..)))
                .collect();
            test_random_push_and_drain_iter(&data);
        }
    }

    fn test_random_push_and_drain_iter(data: &[(u64, u64)]) {
        let mut bulk = TargetedBulk::new();
        for (t, d) in data {
            bulk.push(*t, *d);
        }

        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&results);

        bulk.drain_par(|target, data| {
            let mut results = results_clone.lock().unwrap();
            results.push((target, data));
        });

        let results = results.lock().unwrap();
        assert_eq!(results.len(), data.len());
        for (t, d) in data {
            assert!(results.contains(&(*t, *d)));
        }
    }
}
