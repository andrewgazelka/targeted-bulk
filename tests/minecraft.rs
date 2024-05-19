use evenio::entity::EntityId;
use targeted_bulk::TargetedEvents;

#[test]
fn full() {
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
