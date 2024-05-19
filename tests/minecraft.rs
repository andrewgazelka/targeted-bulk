#![allow(clippy::needless_pass_by_value)]

use derive_more::Constructor;
use evenio::{
    component::Component,
    entity::EntityId,
    event::{GlobalEvent, Receiver},
    world::World,
};
use evenio::component::ComponentDescriptor;
use evenio::mutability::Mutability;
use targeted_bulk::{
    handler::{TargetedReader, TargetedWriter},
    TargetedEvents,
};

#[test]
fn quick() {
    let mut events1 = TargetedEvents::new();

    let events2 = TargetedEvents::new();

    for i in 0..32 {
        let id = EntityId::new(i, 1).unwrap();
        events1.push_exclusive(id, i * 2);
    }

    events1.drain_par(|target, data| {
        events2.push_shared(target, format!("I love {data}"));
    });

    println!("{:#?}", events2);
}

#[derive(Component, Debug)]
pub struct Name(pub String);

#[derive(Component, Debug)]
pub struct Age(pub u8);

#[derive(GlobalEvent)]
struct Call;

#[derive(Constructor)]
struct PrintEvent {
    append: &'static str,
}

struct ShoutEvent {
    to_shout: String,
}

#[test]
fn ecs() {
    let mut world = World::new();

    let id1 = world.spawn();
    world.insert(id1, Name("Alice".into()));
    world.insert(id1, Age(20));

    let id2 = world.spawn();
    world.insert(id2, Name("Bob".into()));
    world.insert(id2, Age(21));

    let id3 = world.spawn();
    world.insert(id3, Name("Charlie".into()));
    world.insert(id3, Age(22));

    let id4 = world.spawn();
    world.insert(id4, Name("David".into()));
    world.insert(id4, Age(23));

    let id = world.spawn();
    let mut print_events = TargetedEvents::new();

    print_events.push_exclusive(id1, PrintEvent::new("in wonderland"));
    print_events.push_exclusive(id2, PrintEvent::new("the builder"));
    print_events.push_exclusive(id3, PrintEvent::new("and the chocolate factory"));
    print_events.push_exclusive(id4, PrintEvent::new("and goliath"));

    world.insert(id, print_events);

    let id = world.spawn();
    let shout_events: TargetedEvents<ShoutEvent> = TargetedEvents::default();
    world.insert(id, shout_events);

    world.add_handler(handler1);
    world.add_handler(handler2);

    world.send(Call);
}

fn handler1(
    _: Receiver<Call>,
    mut reader: TargetedReader<PrintEvent, &Name>,
    writer: TargetedWriter<ShoutEvent>,
) {
    reader.drain_par(|target, data, name| {
        let name = &name.0;
        let append = data.append;
        let to_shout = format!("{name} {append}");
        let event = ShoutEvent { to_shout };
        writer.push_shared(target, event);
    });
}

fn handler2(_: Receiver<Call>, mut reader: TargetedReader<ShoutEvent, &Age>) {
    reader.drain_par(|_, data, age| {
        let age = age.0;
        let to_shout = data.to_shout.to_uppercase();
        println!("{to_shout} is {age}");
    });
}

