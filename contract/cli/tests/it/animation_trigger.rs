//! `animation-trigger` (LLP 1055 D13): an animation in a row a virtualized
//! list mounted out of its port waits, held at its start, until a report
//! puts the row in the port; `none` starts it when the row is inserted.
use exact_kernel::{Kernel, NodeKey};
use exact_motion::Engine;
use exact_plan::Value;
use exact_runner::{
    Advanced, CollectionFeedback, CollectionFill, DataError, DataSource, RowMeasurement, Runner,
};

fn app(trigger: &str) -> String {
    format!(
        "keyframes appear\n  from opacity=0\n  to opacity=1\n\nshape Coin\n  id: number\ncomponent App\n  resource coins = coins() as shape list<Coin>\n  view\n    list virtualized=true height=300 width=400 estimated-item-height=100 testId=\"feed\"\n      each c in coins key=c.id\n        column width=\"100%\" height=100\n          text `${{c.id}}` testId=`coin-${{c.id}}` animation=\"appear 600ms\"{trigger}\n"
    )
}

struct Data;
impl DataSource for Data {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "coins" => Ok(Value::list(
                (0..200)
                    .map(|i| Value::record(vec![Value::Number(i as f64)]))
                    .collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn boot(trigger: &str) -> Runner<Data> {
    let plan = contract::compile(&app(trigger)).unwrap();
    Runner::boot(
        plan,
        Data,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn key(r: &Runner<Data>, test_id: &str) -> Option<NodeKey> {
    r.kernel().find_by_test_id(test_id).first().copied()
}

/// Whether the node's animation is held, as its executors hear the row.
fn held(r: &Runner<Data>, test_id: &str) -> bool {
    let k = r.kernel();
    let node = k.node_by_key(key(r, test_id).unwrap()).unwrap();
    k.animation_row(&node).0.iter().all(|a| a.paused)
}

/// The feed's port at `offset`, every mounted row measured at 100.
fn report(r: &mut Runner<Data>, offset: f64) -> Advanced {
    let k = r.kernel();
    let feed = k.node_by_key(key(r, "feed").unwrap()).unwrap().id;
    let c = r
        .collections()
        .into_iter()
        .find(|c| c.view == feed)
        .unwrap();
    r.collection_feedback_filled(
        CollectionFeedback {
            view: feed,
            revision: c.revision,
            scroll_sequence: c.scroll_sequence + 1,
            offset,
            port_main: 300.0,
            port_cross: 400.0,
            cross: 400.0,
            measurements: c
                .rows
                .iter()
                .map(|row| RowMeasurement {
                    view: row.view,
                    epoch: row.epoch,
                    size: 100.0,
                })
                .collect(),
            focus_view: None,
            interaction_view: None,
        },
        CollectionFill::default(),
    )
    .unwrap()
}

#[test]
fn an_animation_in_a_row_mounted_ahead_waits_for_the_row_to_show() {
    let mut r = boot(" animation-trigger=\"view\"");
    // Twice: the first report measures, the second settles the window.
    report(&mut r, 0.0);
    report(&mut r, 0.0);
    // Rows 0-2 fill the port; the window's lead mounted more below it.
    assert!(!held(&r, "coin-0"), "a row in the port plays");
    assert!(!held(&r, "coin-2"));
    assert!(key(&r, "coin-4").is_some(), "the lead mounted row 4");
    assert!(held(&r, "coin-4"), "a row below the port waits");

    // The engine hears it paused at its start, and running once it shows.
    let mut engine = Engine::new();
    let node = exact_kernel::motion::motion_node(key(&r, "coin-4").unwrap());
    let mut sync = Default::default();
    r.kernel()
        .motion_sync_node(key(&r, "coin-4").unwrap(), &mut sync);
    sync.apply(&mut engine).unwrap();
    engine.advance(5.0).unwrap();
    assert_eq!(engine.animation_plays(node)[0].hold, Some(0.0));

    // Scrolled so row 4 (at 400..500) is in the port (250..550).
    let shown = report(&mut r, 250.0);
    let revealed: Vec<NodeKey> = shown
        .receipts
        .iter()
        .flat_map(|t| t.receipt.revealed.clone())
        .collect();
    assert!(
        !revealed.is_empty(),
        "the commit names the rows that showed"
    );
    assert!(!held(&r, "coin-4"));
    for t in &shown.receipts {
        r.kernel()
            .motion_sync(&t.receipt)
            .apply(&mut engine)
            .unwrap();
    }
    let play = &engine.animation_plays(node)[0];
    assert_eq!(play.hold, None);
    assert_eq!(play.start, 5.0, "it starts when its row shows");

    // A report that changes nothing reveals nothing again.
    let again = report(&mut r, 250.0);
    assert!(again.receipts.iter().all(|t| t.receipt.revealed.is_empty()));
}

#[test]
fn without_the_trigger_a_row_mounted_ahead_plays_from_its_insertion() {
    for trigger in ["", " animation-trigger=\"none\""] {
        let mut r = boot(trigger);
        report(&mut r, 0.0);
        report(&mut r, 0.0);
        assert!(key(&r, "coin-4").is_some());
        assert!(!held(&r, "coin-4"), "{trigger:?}");
        let shown = report(&mut r, 250.0);
        assert!(shown.receipts.iter().all(|t| t.receipt.revealed.is_empty()));
    }
}
