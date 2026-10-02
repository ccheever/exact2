//! LLP 1053.000.000.000: `glassGroup="auto"`, resolved here from the kernel's
//! style — the main axis's gap, the grid's smaller gap, `0` for a block —
//! and sent both as props (`glassGroup`, `glassGroupAuto`) and in the style
//! dictionary (`glass_group_spacing`), so a gap or direction change, which
//! sends no props, reaches Swift.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot(src: &str) -> (Host<NoData>, String) {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap()
}

fn view(host: &Host<NoData>, test_id: &str) -> u32 {
    let k = host.runner().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// The last op of `kind` for `id`, up to the next op, as JSON text.
fn last_op<'a>(batch: &'a str, kind: &str, id: u32) -> Option<&'a str> {
    let at = batch.rfind(&format!("\"op\":\"{kind}\",\"id\":{id},"))?;
    let rest = &batch[at..];
    let end = rest[1..].find("\"op\":").map_or(rest.len(), |n| n + 1);
    Some(&rest[..end])
}

#[test]
fn an_auto_group_takes_its_spacing_from_its_gap_and_follows_it() {
    let (mut host, first) = boot(
        r#"component App
  state wide = false
  state across = true
  action widen
    wide = not wide
  action turn
    across = not across
  view
    column
      row testId="row" glassGroup="auto" column-gap=(wide ? 20 : 8) row-gap=3 flex-direction=(across ? "row" : "column")
        box width=10 height=10
      column testId="column" glassGroup="auto" row-gap=6 column-gap=30
        box width=10 height=10
      box testId="grid" display="grid" glassGroup="auto" row-gap=9 column-gap=4
        box width=10 height=10
      box testId="block" display="block" glassGroup="auto" row-gap=9
        box width=10 height=10
      row testId="fixed" glassGroup=12 column-gap=40
        box width=10 height=10
      button testId="widen" press=widen
        text "widen"
      button testId="turn" press=turn
        text "turn"
"#,
    );
    let (row, column, grid, block, fixed) = (
        view(&host, "row"),
        view(&host, "column"),
        view(&host, "grid"),
        view(&host, "block"),
        view(&host, "fixed"),
    );
    // At creation: the props carry the points and the flag, the style the points.
    for (id, points) in [(row, "8"), (column, "6"), (grid, "4"), (block, "0")] {
        let create = last_op(&first, "create", id).unwrap();
        assert!(
            create.contains(&format!("\"glassGroup\":\"{points}\"")),
            "{id}: {create}"
        );
        assert!(create.contains("\"glassGroupAuto\":\"true\""), "{create}");
        assert!(
            create.contains(&format!("\"glass_group_spacing\":{points}")),
            "{id}: {create}"
        );
    }
    // A number is a number: no flag, no style key.
    let create = last_op(&first, "create", fixed).unwrap();
    assert!(create.contains("\"glassGroup\":\"12\""), "{create}");
    assert!(!create.contains("glassGroupAuto"), "{create}");
    assert!(!create.contains("glass_group_spacing"), "{create}");
    // A gap change sends no props, and the style op carries the new spacing.
    let batch = host.dispatch_at(view(&host, "widen"), Event::Press, 0.0);
    let style = last_op(&batch, "style", row).unwrap_or_else(|| panic!("{batch}"));
    assert!(style.contains("\"glass_group_spacing\":20"), "{style}");
    // A direction change: the row becomes a column, its spacing its row gap.
    let batch = host.dispatch_at(view(&host, "turn"), Event::Press, 0.0);
    let style = last_op(&batch, "style", row).unwrap_or_else(|| panic!("{batch}"));
    assert!(style.contains("\"glass_group_spacing\":3"), "{style}");
}
