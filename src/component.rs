use std::alloc::Layout;

use derive_more::{Deref, DerefMut};
use evenio::{
    component::{Component, ComponentDescriptor, ComponentId},
    entity::EntityId,
    event::Spawn,
    fetch::Single,
    mutability::Mutability,
    prelude::{Receiver, World},
};

use crate::get_thread_index;

#[derive(Component, Deref, DerefMut)]
#[component(immutable)]
struct ThreadComponentLookup {
    thread_to_component_idx: Vec<ComponentId>,
}

fn init_components() -> ThreadComponentLookup {
    let num_threads = rayon::current_num_threads();

    let mut world = World::new();

    let thread_to_component_idx: Vec<_> = (0..num_threads)
        .map(|i| init_component(&mut world, i))
        .collect();

    ThreadComponentLookup {
        thread_to_component_idx,
    }
}

fn init_component(world: &mut World, idx: usize) -> ComponentId {
    let name = format!("thread_{idx}").into();

    let descriptor = ComponentDescriptor {
        name,
        type_id: None,
        layout: Layout::new::<()>(),
        drop: None,
        mutability: Mutability::Immutable,
    };

    unsafe { world.add_component_with_descriptor(descriptor) }
}

fn tag_system(s: Single<&ThreadComponentLookup>, r: Receiver<Spawn, EntityId>) {
    let id = r.query;
    let thread_idx = get_thread_index(id);

    let component = s.thread_to_component_idx.get(thread_idx).unwrap();

    // todo: how to impl
}
