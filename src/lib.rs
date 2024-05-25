#![feature(allocator_api)]

use std::{cell::UnsafeCell, fmt::Debug, marker::PhantomData, thread::ThreadId};

use evenio::{component::Component, entity::EntityId};

pub mod handler;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub struct ThreadPinned<T> {
    data: T,
    /// So [`ThreadPinned`] is not [`Send`] nor [`Sync`].
    _marker: PhantomData<*const ()>,
}

impl<T> ThreadPinned<T> {
    #[must_use]
    pub const fn inner(&self) -> &T {
        &self.data
    }

    const fn new(data: T) -> Self {
        Self {
            data,
            _marker: PhantomData,
        }
    }
}

/// # Safety
/// Must be a pure function that returns the same value for the same input.
pub unsafe trait RayonThreadAssignable {
    fn get_rayon_thread_index(&self) -> usize;
}

macro_rules! impl_rayon_thread_assignable_for_primitive {
    ($($t:ty),*) => {
        $(
            // this is fine, we just want a consistent mapping
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            unsafe impl RayonThreadAssignable for $t {
                fn get_rayon_thread_index(&self) -> usize {
                    get_rayon_thread_index(*self as usize)
                }
            }
        )*
    };
}

impl_rayon_thread_assignable_for_primitive!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

unsafe impl RayonThreadAssignable for EntityId {
    fn get_rayon_thread_index(&self) -> usize {
        get_rayon_thread_index(self.index().0 as usize)
    }
}

fn get_rayon_thread_index(id: usize) -> usize {
    id % rayon::current_num_threads()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalEvents<E, T> {
    pinned_thread_id: Option<ThreadId>,
    targets: Vec<T>,
    events: Vec<E>,
}

impl<E, T> LocalEvents<E, T> {
    const EMPTY: Self = Self {
        targets: Vec::new(),
        events: Vec::new(),
        pinned_thread_id: None,
    };

    /// Assert that the current thread is the same as the one that was pinned to the local events.
    fn assert_correct_pinned_thread(&mut self) {
        let current = std::thread::current().id();
        if let Some(thread_id) = self.pinned_thread_id {
            assert_eq!(
                current, thread_id,
                "current thread id does not match the one that was pinned to the local events"
            );
        } else {
            self.pinned_thread_id = Some(current);
        }
    }
}

#[derive(Debug, Component)]
pub struct TargetedEvents<E, T = EntityId> {
    locals: Box<[UnsafeCell<LocalEvents<E, T>>]>,
}

unsafe impl<E, T> Send for TargetedEvents<E, T> {}
unsafe impl<E, T> Sync for TargetedEvents<E, T> {}

impl<E, T> Default for TargetedEvents<E, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, T> TargetedEvents<E, T> {
    #[must_use]
    pub fn new() -> Self {
        rayon::current_num_threads();

        let num_threads = rayon::current_num_threads();

        let locals = (0..num_threads)
            .map(|_| LocalEvents::EMPTY)
            .map(UnsafeCell::new)
            .collect();

        Self { locals }
    }

    pub fn len(&mut self) -> usize {
        self.locals
            .iter_mut()
            .map(UnsafeCell::get_mut)
            .map(|local| local.len())
            .sum()
    }

    pub fn is_empty(&mut self) -> bool {
        self.locals
            .iter_mut()
            .map(UnsafeCell::get_mut)
            .all(|local| local.is_empty())
    }

    pub fn push_exclusive(&mut self, target: T, data: E)
    where
        T: RayonThreadAssignable,
    {
        let idx = target.get_rayon_thread_index();

        let locals = self
            .locals
            .get_mut(idx)
            .expect("thread index out of bounds; did you create multiple rayon ThreadPools?");

        let locals = locals.get_mut();
        locals.push(target, data);
    }

    /// # Panics
    /// Panics if the target's assigned thread index does not match the current thread index.
    pub fn push_shared(&self, target: ThreadPinned<T>, data: E) {
        let local = self.get_current_local();
        let local = unsafe { &mut *local.get() };
        local.push(target.data, data);
    }

    /// # Safety
    fn get_current_local(&self) -> &UnsafeCell<LocalEvents<E, T>> {
        let current_thread_index = rayon::current_thread_index().expect("not in a rayon thread");
        let local = self.get_local(current_thread_index);

        {
            let local = unsafe { &mut *local.get() };
            local.assert_correct_pinned_thread();
        }

        local
    }

    fn get_local(&self, index: usize) -> &UnsafeCell<LocalEvents<E, T>> {
        self.locals
            .get(index)
            .expect("thread index out of bounds; did you create multiple rayon ThreadPools?")
    }

    // this requires a `&mut self` reference for safety
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn drain_par<F>(&mut self, process: F)
    where
        F: Fn(ThreadPinned<T>, E) + Send + Sync,
        E: Send + Sync,
    {
        rayon::broadcast(|_| {
            let local = self.get_current_local();
            let local = unsafe { &mut *local.get() };

            let targets = local.targets.drain(..);
            let events = local.events.drain(..);

            targets.zip(events).for_each(|(target, data)| {
                let target = ThreadPinned::new(target);
                process(target, data);
            });
        });
    }
}

impl<E, T> Default for LocalEvents<E, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, T> LocalEvents<E, T> {
    #[must_use]
    const fn new() -> Self {
        Self {
            pinned_thread_id: None,
            targets: Vec::new(),
            events: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.targets.len()
    }

    fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    fn push(&mut self, target: T, data: E) {
        self.targets.push(target);
        self.events.push(data);
    }
}
