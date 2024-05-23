#![feature(allocator_api)]

use std::{
    cell::UnsafeCell,
    fmt::{Debug, Formatter},
    marker::PhantomData,
    thread::ThreadId,
};

use evenio::{component::Component, entity::EntityId};

pub mod handler;
pub mod component;

#[derive(PartialEq, Eq, Copy, Clone)]
pub struct TargetedId {
    data: EntityId,
    /// So [`TargetedId`] is not [`Send`] nor [`Sync`].
    _marker: PhantomData<*const ()>,
}

impl Debug for TargetedId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TargetedId").field(&self.data).finish()
    }
}

impl TargetedId {
    #[must_use]
    pub const fn inner(self) -> EntityId {
        self.data
    }

    const fn new(data: EntityId) -> Self {
        Self {
            data,
            _marker: PhantomData,
        }
    }
}

fn get_rayon_thread_index(id: EntityId) -> usize {
    id.index().0 as usize % rayon::current_num_threads()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalEvents<E> {
    pinned_thread_id: Option<ThreadId>,
    targets: Vec<EntityId>,
    events: Vec<E>,
}

impl<E> LocalEvents<E> {
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
pub struct TargetedEvents<E> {
    locals: Box<[UnsafeCell<LocalEvents<E>>]>,
}

unsafe impl<E> Send for TargetedEvents<E> {}
unsafe impl<E> Sync for TargetedEvents<E> {}

impl<E> Default for TargetedEvents<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> TargetedEvents<E> {
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

    pub fn push_exclusive(&mut self, target: EntityId, data: E) {
        let idx = get_rayon_thread_index(target);

        let locals = self
            .locals
            .get_mut(idx)
            .expect("thread index out of bounds; did you create multiple rayon ThreadPools?");

        let locals = locals.get_mut();
        locals.push(target, data);
    }

    /// # Panics
    /// Panics if the target's assigned thread index does not match the current thread index.
    pub fn push_shared(&self, target: TargetedId, data: E) {
        let local = self.get_current_local();
        let local = unsafe { &mut *local.get() };
        local.push(target.data, data);
    }

    /// # Safety
    fn get_current_local(&self) -> &UnsafeCell<LocalEvents<E>> {
        let current_thread_index = rayon::current_thread_index().expect("not in a rayon thread");
        let local = self.get_local(current_thread_index);

        {
            let local = unsafe { &mut *local.get() };
            local.assert_correct_pinned_thread();
        }

        local
    }

    fn get_local(&self, index: usize) -> &UnsafeCell<LocalEvents<E>> {
        self.locals
            .get(index)
            .expect("thread index out of bounds; did you create multiple rayon ThreadPools?")
    }

    // this requires a `&mut self` reference for safety
    #[allow(clippy::needless_pass_by_ref_mut)]
    pub fn drain_par<F>(&mut self, process: F)
    where
        F: Fn(TargetedId, E) + Send + Sync,
        E: Send + Sync,
    {
        rayon::broadcast(|_| {
            let local = self.get_current_local();
            let local = unsafe { &mut *local.get() };

            let targets = local.targets.drain(..);
            let events = local.events.drain(..);

            targets.zip(events).for_each(|(target, data)| {
                let target = TargetedId::new(target);
                process(target, data);
            });
        });
    }
}

impl<E> Default for LocalEvents<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> LocalEvents<E> {
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

    fn push(&mut self, target: EntityId, data: E) {
        self.targets.push(target);
        self.events.push(data);
    }
}
