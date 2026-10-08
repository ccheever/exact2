use super::*;
use exact_kernel::{NodeType, ViewId};
use exact_plan::Value;
use exact_runner::{CollectionFeedback, DataError, Event};
use serde_json::{json, Value as Json};

struct Rows;
impl DataSource for Rows {
    fn query(&mut self, _: &str, _: &[Value]) -> Result<Value, DataError> {
        Ok(Value::list(
            (0..400).map(|i| Value::Number(i as f64)).collect(),
        ))
    }
}
const SOURCE: &str = r##"component App
  resource rows = rows() as shape list<number>
  view
    list virtualized=true height=320 width=240 estimated-item-height=64
      each row in rows key=row
        Row(x=row)
component Row
  props
    x: number
  state open = false
  action toggle
    open = not open
  view
    column height=64 testId=`row-${x}` press=toggle color=(open ? "#ffffff" : "#000000") transition="color 1s linear"
      text
        text `${x}` testId=`run-${x}`
      when x < 30
        text "first"
      else
        image `assets/${x}.png` width=10 height=10
"##;
fn boot(reuse: bool) -> Host<Rows> {
    let (mut host, _) = Host::boot(
        &contract::compile(SOURCE).unwrap().encode(),
        Rows,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        240.,
        320.,
    )
    .unwrap();
    host.set_row_reuse(reuse);
    host
}
fn feedback(host: &mut Host<Rows>, offset: f64, at: f64) -> Json {
    let c = &host.runner().collections()[0];
    let bytes = CollectionFeedback {
        view: c.view,
        revision: c.revision,
        scroll_sequence: c.scroll_sequence + 1,
        offset,
        port_cross: 240.,
        port_main: 320.,
        cross: 240.,
        measurements: vec![],
        focus_view: None,
        interaction_view: None,
    }
    .encode()
    .unwrap();
    let batch: Json = serde_json::from_str(&host.collection_feedback(&bytes, at)).unwrap();
    assert!(batch["error"].is_null(), "{batch}");
    batch
}
fn ids(batch: &Json) -> BTreeSet<ViewId> {
    batch["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["op"] == "renew")
        .flat_map(|op| op["ids"].as_array().unwrap())
        .map(|id| id.as_u64().unwrap() as u32)
        .collect()
}

#[test]
fn row_reuse_reset_precedes_deltas_and_excludes_new_arms_and_inline_runs() {
    let mut host = boot(true);
    feedback(&mut host, 0., 0.);
    let carriers: BTreeSet<_> = host.mirror.keys().copied().collect();
    let inline: BTreeSet<_> = host.inline_runs.keys().copied().collect();
    let before = host.runner().collections()[0].rows.clone();
    let batch = feedback(&mut host, 2400., 0.);
    assert_eq!(batch["ops"][0]["op"], "renew");
    let renewed = ids(&batch);
    assert!(!renewed.is_empty());
    assert!(renewed.is_subset(&carriers));
    assert!(renewed.is_disjoint(&inline));
    assert!(
        batch["ops"]
            .as_array()
            .unwrap()
            .iter()
            .any(|op| op["op"] == "create"),
        "changed arms must still create their new image carriers"
    );
    for row in &host.runner().collections()[0].rows {
        if let Some(old) = before
            .iter()
            .find(|old| old.view == row.view && old.index != row.index)
        {
            assert_ne!(old.epoch, row.epoch);
            assert!(renewed.contains(&row.view));
        }
    }
}

#[test]
fn row_reuse_clears_old_inline_animation_paint_before_new_paragraphs() {
    let mut host = boot(true);
    feedback(&mut host, 0., 0.);
    let row_roots: Vec<_> = host.runner().collections()[0]
        .rows
        .iter()
        .map(|row| row.root)
        .collect();
    for row in row_roots {
        host.dispatch_at(row, Event::Press, 0.);
    }
    host.tick(500.);
    let old_runs: BTreeSet<_> = host.paint.runs.keys().copied().collect();
    assert!(
        !old_runs.is_empty(),
        "fixture must actually animate inherited inline paint"
    );
    let batch = feedback(&mut host, 2400., 500.);
    assert!(!ids(&batch).is_empty());
    let mut retained_runs = 0;
    for op in batch["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["op"] == "paragraph")
    {
        for run in op["runs"].as_array().unwrap() {
            let id = run["id"].as_u64().unwrap() as u32;
            if !old_runs.contains(&id) {
                continue;
            }
            retained_runs += 1;
            let node = host.runner().kernel().node(id).unwrap();
            let target = crate::style::style_json_for(&node, &host.runner().kernel().env()).0;
            assert_eq!(run["style"], serde_json::from_str::<Json>(&target).unwrap());
        }
    }
    assert!(
        retained_runs > 0,
        "a formerly animated inline run must be rebound"
    );
}

#[test]
fn row_reuse_unchanged_authored_content_is_retained_and_opt_out_keeps_fresh_mounts() {
    let source = SOURCE.replace("`${x}`", "\"same\"");
    let (mut host, _) = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        Rows,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        240.,
        320.,
    )
    .unwrap();
    host.set_row_reuse(true);
    feedback(&mut host, 0., 0.);
    let batch = feedback(&mut host, 640., 0.);
    let renewed = ids(&batch);
    assert!(!renewed.is_empty());
    for id in &renewed {
        let node = host.runner().kernel().node(*id).unwrap();
        assert!(host.mirror.contains_key(id));
        if node.node_type == NodeType::Text {
            assert_eq!(host.mirror[id].props, host.props_of(&node));
        }
    }
    host.set_row_reuse(false);
    assert!(ids(&feedback(&mut host, 20000., 0.)).is_empty());
    let mut default = boot(false);
    feedback(&mut default, 0., 0.);
    assert!(ids(&feedback(&mut default, 2000., 0.)).is_empty());
}

#[test]
fn renew_only_batch_is_nonempty_and_reset_is_first_even_after_staged_operations() {
    let mut batch = Batch::new();
    batch.props(9, &[("testId", "next".into())], &[]);
    batch.renew(&[9]);
    assert!(!batch.is_empty());
    let value: Json = serde_json::from_str(&batch.finish(None, false, 0., None)).unwrap();
    assert_eq!(value["ops"][0], json!({"op":"renew","ids":[9]}));
    assert_eq!(value["ops"][1]["op"], "props");
    let mut batch = Batch::new();
    batch.renew(&[9]);
    assert!(!batch.is_empty());
}

#[test]
fn row_reuse_changed_image_source_forgets_old_natural_size_before_layout() {
    let source="component App\n  resource rows = rows() as shape list<number>\n  view\n    list virtualized=true height=320 width=240 estimated-item-height=64\n      each row in rows key=row\n        column height=64\n          image `assets/${row}.png` width=100\n";
    let bytes = contract::compile(source).unwrap().encode();
    let make = || {
        Host::boot(
            &bytes,
            Rows,
            Box::new(exact_kernel::MonospaceMeasurer::default()),
            240.,
            320.,
        )
        .unwrap()
        .0
    };
    let (mut fresh, mut reused) = (make(), make());
    reused.set_row_reuse(true);
    feedback(&mut fresh, 0., 0.);
    feedback(&mut reused, 0., 0.);
    let images: Vec<_> = reused
        .keys
        .values()
        .copied()
        .filter_map(|id| {
            let node = reused.runner().kernel().node(id).unwrap();
            (node.node_type == NodeType::Image).then(|| {
                (
                    id,
                    node.props
                        .str(exact_kernel::PropId::ImageSource)
                        .unwrap()
                        .to_string(),
                )
            })
        })
        .collect();
    assert!(!images.is_empty());
    for (id, _) in &images {
        let batch: Json =
            serde_json::from_str(&reused.set_intrinsic(*id, Some((400., 100.)))).unwrap();
        assert!(batch["error"].is_null(), "{batch}");
    }
    feedback(&mut fresh, 2400., 0.);
    let batch = feedback(&mut reused, 2400., 0.);
    let mut retained_images = 0;
    for id in ids(&batch) {
        let Some((_, old_source)) = images.iter().find(|(old, _)| *old == id) else {
            continue;
        };
        retained_images += 1;
        let k = reused.runner().kernel();
        let node = k.node(id).unwrap();
        assert_ne!(
            node.props.str(exact_kernel::PropId::ImageSource).unwrap(),
            old_source
        );
        assert_eq!(k.arena().intrinsic(k.arena().slot_of(id).unwrap()), None);
    }
    assert!(retained_images > 0);
    for row in &fresh.runner().collections()[0].rows {
        let same = reused.runner().collections()[0]
            .rows
            .iter()
            .find(|r| r.index == row.index)
            .unwrap()
            .clone();
        let image = |h: &Host<Rows>, root| h.runner().kernel().node(root).unwrap().children()[0];
        let a = fresh
            .runner()
            .kernel()
            .node(image(&fresh, row.root))
            .unwrap()
            .frame;
        let b = reused
            .runner()
            .kernel()
            .node(image(&reused, same.root))
            .unwrap()
            .frame;
        assert_eq!((a.width, a.height), (b.width, b.height));
    }
}

#[test]
fn row_reuse_same_image_source_keeps_immutable_natural_size_metadata() {
    let source="component App\n  resource rows = rows() as shape list<number>\n  view\n    list virtualized=true height=320 width=240 estimated-item-height=64\n      each row in rows key=row\n        column height=64\n          image \"assets/shared.png\" width=100\n";
    let (mut host, _) = Host::boot(
        &contract::compile(source).unwrap().encode(),
        Rows,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        240.,
        320.,
    )
    .unwrap();
    host.set_row_reuse(true);
    feedback(&mut host, 0., 0.);
    let images: Vec<_> = host
        .keys
        .values()
        .copied()
        .filter(|id| host.runner().kernel().node(*id).unwrap().node_type == NodeType::Image)
        .collect();
    for id in &images {
        host.set_intrinsic(*id, Some((400., 100.)));
    }
    let batch = feedback(&mut host, 2400., 0.);
    let retained = ids(&batch)
        .intersection(&images.iter().copied().collect())
        .copied()
        .collect::<Vec<_>>();
    assert!(!retained.is_empty());
    for id in retained {
        let k = host.runner().kernel();
        assert_eq!(
            k.arena().intrinsic(k.arena().slot_of(id).unwrap()),
            Some((400., 100.))
        );
        assert_eq!(
            k.node(id)
                .unwrap()
                .props
                .str(exact_kernel::PropId::ImageSource),
            Some("assets/shared.png")
        );
    }
}

#[test]
fn row_reuse_identity_reset_skips_redundant_motion_and_keeps_nonidentity_targets() {
    let source = SOURCE.replace(
        "column height=64 testId=",
        "column opacity=0.5 height=64 testId=",
    );
    let (mut host, _) = Host::boot(
        &contract::compile(&source).unwrap().encode(),
        Rows,
        Box::new(exact_kernel::MonospaceMeasurer::default()),
        240.,
        320.,
    )
    .unwrap();
    host.set_row_reuse(true);
    feedback(&mut host, 0., 0.);
    let batch = feedback(&mut host, 2400., 0.);
    let renewed = ids(&batch);
    assert!(!renewed.is_empty());
    let mut nonidentity = 0;
    for op in batch["ops"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|op| op["op"] == "present")
    {
        let id = op["id"].as_u64().unwrap() as u32;
        if !renewed.contains(&id) {
            continue;
        }
        assert_eq!(
            op["property"], "opacity",
            "fresh identity should require no delta: {op}"
        );
        assert_eq!(op["x"], 0.5);
        nonidentity += 1;
    }
    let roots = host.runner().collections()[0]
        .rows
        .iter()
        .filter(|row| renewed.contains(&row.root))
        .count();
    assert_eq!(
        nonidentity, roots,
        "every reset row must regain its authored nonidentity opacity"
    );
    assert!(nonidentity > 0);
    let mut batch = Batch::new();
    batch.renew(&[7, 2, 7]);
    assert!(batch.starts_presentation(2));
    assert!(batch.starts_presentation(7));
    assert!(
        !batch.creates(2),
        "logical renewal must not masquerade as platform creation"
    );
}
