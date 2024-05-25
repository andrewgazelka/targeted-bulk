use evenio::{
    entity::EntityId,
    fetch::{Fetcher, Single},
    handler::HandlerParam,
    query::Query,
};

use crate::{ConsistentKey, TargetedEvents, ThreadPinned};

// todo: can we remove 'static bound?
// todo: how evenio use TypeId without 'static bound?
#[derive(HandlerParam)]
pub struct TargetedReader<'a, E: 'static, Q: Query + 'static, T: 'static = EntityId> {
    events: Single<'a, &'static mut TargetedEvents<E, T>>,
    query: Fetcher<'a, Q>,
}

impl<'a, E: 'static, Q: Query + 'static> TargetedReader<'a, E, Q, EntityId> {
    // todo: is there a way to avoid this?
    #[allow(clippy::mut_mut)]
    pub fn drain_par<'b, F>(&'b mut self, process: F)
    where
        F: Fn(ThreadPinned<EntityId>, E, Q::Item<'b>) + Send + Sync,
        E: Send + Sync,
        Q::Item<'b>: Send + Sync,
    {
        let events = &mut self.events.0;
        let query = &mut self.query;

        events.drain_par(|target, data| {
            let item = unsafe { query.get_unchecked(*target.inner()) };

            let Ok(item) = item else {
                return;
            };

            process(target, data, item);
        });
    }
}

#[derive(HandlerParam)]
pub struct TargetedWriter<'a, E: 'static, T: 'static = EntityId> {
    events: Single<'a, &'static mut TargetedEvents<E, T>>,
}

impl<'a, E: 'static, T: 'static> TargetedWriter<'a, E, T> {
    pub fn push_exclusive(&mut self, target: T, data: E)
    where
        T: ConsistentKey,
    {
        self.events.0.push_exclusive(target, data);
    }

    pub fn push_shared(&self, target: ThreadPinned<T>, data: E) {
        self.events.0.push_shared(target, data);
    }
}
