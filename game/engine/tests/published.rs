//! A large public record (Grow a Garden's whole backpack, diary 004 limit 2):
//! a change to one small field re-encodes only that field, and the journal
//! keeps a bounded line for a large one.
use exact_game::*;

struct Empty;
impl Game for Empty {
    type Args = ();
    const ID: &'static str = "published";
    fn setup(_: &mut World, _: &()) {}
    fn tick(_: &mut World, _: &Input, _: &()) {}
}
#[derive(Default, Data, Clone)]
struct Fruit {
    id: u32,
    label: String,
    weight: f64,
    value: u32,
    mutated: bool,
}
#[derive(Default, Data)]
struct Hud {
    status: String,
    backpack: Vec<Fruit>,
}
fn backpack(n: u32) -> Vec<Fruit> {
    (0..n)
        .map(|id| Fruit {
            id,
            label: format!("Golden carrot {id}"),
            weight: 1.25 + id as f64 / 7.0,
            value: 30 + id % 17,
            mutated: id % 5 == 0,
        })
        .collect()
}

#[test]
fn a_large_field_is_journaled_bounded_and_the_record_stays_exact() {
    let mut sim = Sim::<Empty>::new(()).unwrap();
    let hud = Hud {
        status: "day 1".into(),
        backpack: backpack(2_000),
    };
    sim.world().publish_record(&hud);
    let whole = sim.take_published().unwrap();
    assert!(whole.len() > 100_000, "{}", whole.len());
    assert!(whole.ends_with(r#"],"status":"day 1"}"#));
    let line = sim
        .world()
        .journal()
        .into_iter()
        .map(|e| e.line)
        .find(|l| l.contains("publish backpack:"))
        .unwrap();
    assert!(line.len() < 4_200, "journal line is {} bytes", line.len());
    assert!(line.ends_with(" bytes)"), "{}", &line[line.len() - 40..]);
    // One small field changes: the record is the old one with it replaced.
    sim.world().publish_record(&Hud {
        status: "day 2".into(),
        ..hud
    });
    let next = sim.take_published().unwrap();
    assert_eq!(next, whole.replace("day 1", "day 2"));
    // A save and restore carry the same record.
    let saved = sim.save().unwrap();
    let mut restored = Sim::<Empty>::new(()).unwrap();
    restored.restore(&saved).unwrap();
    assert_eq!(restored.take_published().as_deref(), Some(next.as_str()));
}

/// `cargo test --release -p exact-game --test published -- --ignored --nocapture`:
/// the garden's 69,327-fruit backpack (~6.4 MB) published beside a status
/// line that changes once a garden second; the time to take the record after
/// one status change.
#[test]
#[ignore = "measurement"]
fn status_change_beside_a_whole_backpack() {
    let mut sim = Sim::<Empty>::new(()).unwrap();
    let rows = backpack(69_327);
    sim.world().publish_record(&Hud {
        status: "t=0".into(),
        backpack: rows.clone(),
    });
    let bytes = sim.take_published().unwrap().len();
    let rounds = 20;
    let start = std::time::Instant::now();
    for i in 1..=rounds {
        sim.world().publish("status", format!("t={i}").as_str());
        assert!(sim.take_published().is_some());
    }
    let per = start.elapsed().as_secs_f64() * 1e3 / rounds as f64;
    println!("record {bytes} bytes: {per:.2} ms per status change");
}
