#![feature(allocator_api)]

use std::{
    fmt::{Debug, Formatter},
    marker::PhantomData,
};

use evenio::{component::Component, entity::EntityId};

pub mod handler;

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

fn get_thread_index(id: EntityId) -> usize {
    id.index().0 as usize % rayon::current_num_threads()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct LocalEvents<E> {
    targets: Vec<EntityId>,
    events: Vec<E>,
}

impl<E> LocalEvents<E> {
    const EMPTY: Self = Self {
        targets: Vec::new(),
        events: Vec::new(),
    };
}

#[derive(Debug, PartialEq, Eq, Clone, Component)]
pub struct TargetedEvents<E> {
    locals: Box<[LocalEvents<E>]>,
}

impl<E> Default for TargetedEvents<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> TargetedEvents<E> {
    #[must_use]
    pub fn new() -> Self {
        let num_threads = rayon::current_num_threads();

        let locals = (0..num_threads).map(|_| LocalEvents::EMPTY).collect();

        Self { locals }
    }

    pub fn len(&self) -> usize {
        self.locals.iter().map(LocalEvents::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.locals.iter().all(LocalEvents::is_empty)
    }

    pub fn push_exclusive(&mut self, target: EntityId, data: E) {
        let idx = get_thread_index(target);
        self.locals[idx].push(target, data);
    }

    /// # Panics
    /// Panics if the target's assigned thread index does not match the current thread index.
    pub fn push_shared(&self, target: TargetedId, data: E) {
        let ptr = self.locals.as_ptr();
        let current_thread_index = rayon::current_thread_index().expect("not in a rayon thread");
        let ptr = unsafe { ptr.add(current_thread_index) };
        let ptr = ptr.cast_mut();
        let local = unsafe { &mut *ptr };
        local.push(target.data, data);
    }

    pub fn drain_par<F>(&mut self, process: F)
    where
        F: Fn(TargetedId, E) + Send + Sync,
        E: Send + Sync,
    {
        rayon::broadcast(|ctx| {
            let index = ctx.index();
            let local = unsafe { self.locals.as_ptr().add(index) };
            let local = local.cast_mut();
            let local = unsafe { &mut *local };

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
