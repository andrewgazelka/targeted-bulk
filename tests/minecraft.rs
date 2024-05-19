use evenio::{
    component::Component,
    entity::EntityId,
    event::{GlobalEvent, Receiver},
    world::World,
};
use targeted_bulk::{handler::TargetedReader, TargetedEvents};

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
    let mut events = TargetedEvents::new();

    events.push_exclusive(id1, "in wonderland");
    events.push_exclusive(id2, "the builder");
    events.push_exclusive(id3, "and the chocolate factory");
    events.push_exclusive(id4, "and goliath");

    world.insert(id, events);

    world.add_handler(process_handler);

    world.send(Call);
}

fn process_handler(_: Receiver<Call>, mut reader: TargetedReader<&'static str, (&Name, &Age)>) {
    println!("called");
    reader.drain_par(|target, data, (name, age)| {
        println!("{target:?} {name:?} {data} {age:?}");
    });
}
