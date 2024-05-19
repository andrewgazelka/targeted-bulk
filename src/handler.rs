use evenio::{
    fetch::{Fetcher, Single},
    handler::HandlerParam,
    query::Query,
};

use crate::{TargetedEvents, TargetedId};

// todo: can we remove 'static bound?
// todo: how evenio use TypeId without 'static bound?
#[derive(HandlerParam)]
pub struct TargetedReader<'a, E: 'static, Q: Query + 'static> {
    events: Single<'a, &'static mut TargetedEvents<E>>,
    query: Fetcher<'a, Q>,
}

impl<'a, E: 'static, Q: Query + 'static> TargetedReader<'a, E, Q> {
    // todo: is there a way to avoid this?
    #[allow(clippy::mut_mut)]
    pub fn drain_par<'b, F>(&'b mut self, process: F)
    where
        F: Fn(TargetedId, E, Q::Item<'b>) + Send + Sync,
        E: Send + Sync,
        Q::Item<'b>: Send + Sync,
    {
        let events = &mut self.events.0;
        let query = &mut self.query;

        events.drain_par(|target, data| {
            let item = unsafe { query.get_unchecked(target.inner()) };

            let Ok(item) = item else {
                return;
            };

            process(target, data, item);
        });
    }
}

#[derive(HandlerParam)]
pub struct TargetedWriter<'a, E: 'static> {
    events: Single<'a, &'static mut TargetedEvents<E>>,
}

impl<'a, E: 'static> TargetedWriter<'a, E> {
    pub fn push_exclusive(&mut self, target: TargetedId, data: E) {
        self.events.0.push_exclusive(target.inner(), data);
    }

    pub fn push_shared(&self, target: TargetedId, data: E) {
        self.events.0.push_shared(target, data);
    }
}
