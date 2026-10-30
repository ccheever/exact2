//! A node's opacity presented when it mounted is presented again when a later
//! commit changes it.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

const SRC: &str = r##"shape Host
  id: string

component App
  state busy = false
  state editing = true
  state hosts: Host[] = []
  action save
    busy = true
    hosts = [Host(id="a")]
  action done
    busy = false
    editing = false
  view
    column position="relative"
      button press=done testId="done"
        text "Done"
      for host in hosts key=host.id
        row flex-shrink=0 gap="0.25rem"
          EasGhostButton(label="Edit", destructive=false, disabled=busy, buttonId=`eas-host-edit-${host.id}`)
          EasGhostButton(label="Remove", destructive=true, disabled=busy, buttonId=`eas-host-remove-${host.id}`)
      if editing
        column position="absolute" left=0 right=0 top=0 bottom=100 z-index=30
          button press=save disabled=busy testId="save" opacity=(busy ? 0.64 : 1) data-focus-stop=(busy ? "" : "enabled")
            if busy
              box width="0.75rem" height="0.75rem" animation="spin 1s linear infinite"
            text "Save host"

keyframes spin
  from rotate=0
  to rotate=360

// The T3 Code clone's EasGhostButton before b9e83df3a5, every property.
component EasGhostButton
  props
    label: string
    destructive: bool
    disabled: bool
    buttonId: string
  state over = false
  action hover(value: bool)
    over = value
  derive lit = over and not disabled
  view
    button id=buttonId hover=hover disabled=disabled testId=buttonId display="flex" flex-direction="row" align-items="center" height="1.75rem" padding-left="0.5625rem" padding-right="0.5625rem" box-sizing="border-box" border-radius="0.5rem" border-width=0 background-color=(lit ? "light-dark(#f4f4f5, #ffffff0a)" : "#00000000") opacity=(disabled ? 0.64 : 1) data-focus-stop=(disabled ? "" : "enabled page")
      :active
        background-color="light-dark(#f4f4f5, #ffffff0a)"
      text label font-size="0.875rem" font-weight=500 line-height="1.25rem" color=(destructive ? (lit ? "light-dark(#fb2c36, #fb414a)" : "light-dark(#71717b, #a1a1a1)") : "light-dark(#18181b, #f5f5f5)") white-space="nowrap"
"##;

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> Option<u32> {
    let k = host.runner().kernel();
    let key = *k.find_by_test_id(test_id).first()?;
    Some(k.node_by_key(key).unwrap().id)
}

/// The last `present` of `opacity` for `id` in a batch.
fn opacity(batch: &str, id: u32) -> Option<f64> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"opacity\",\"x\":");
    let at = batch.rfind(&marker)?;
    batch[at + marker.len()..]
        .split([',', '}'])
        .next()?
        .parse()
        .ok()
}

#[test]
fn a_row_mounted_dimmed_is_presented_whole_once_its_opacity_returns_to_one() {
    let plan = contract::compile(SRC).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let (save, done) = (view(&host, "save").unwrap(), view(&host, "done").unwrap());
    let mounted = host.dispatch_at(save, Event::Press, 100.0);
    let tests = ["eas-host-edit-a", "eas-host-remove-a"];
    let ids = tests.map(|t| view(&host, t).unwrap());
    for id in ids {
        assert!(
            mounted.contains(&format!("\"op\":\"create\",\"id\":{id},")),
            "{mounted}"
        );
        let dimmed = opacity(&mounted, id).expect("mounted at 0.64");
        assert!((dimmed - 0.64).abs() < 1e-6, "{dimmed}");
    }
    let ended = host.dispatch_at(done, Event::Press, 200.0);
    for (test, id) in tests.into_iter().zip(ids) {
        assert_eq!(view(&host, test), Some(id), "the row stays mounted");
        assert_eq!(opacity(&ended, id), Some(1.0), "{ended}");
    }
}

/// The save's end lands in the batch that mounted the row: the row's first
/// layout raises a `resize` round (an element resize event's first
/// observation), whose commit lifts `busy`. The round's presentation is
/// written into the same batch after the mount's `present opacity 0.64`, so
/// the value back at identity must be presented too: the presenter does not
/// start that view at identity any more. The T3 Code clone's saved EAS host
/// took its end this way, through its timers due at the round.
const ROUND: &str = r##"shape Host
  id: string

component App
  state busy = false
  state hosts: Host[] = []
  action save
    busy = true
    hosts = [Host(id="a")]
  action laidOut(width: number, height: number)
    busy = false
  view
    column
      button press=save testId="save"
        text "Save"
      for host in hosts key=host.id
        row
          button disabled=busy testId=`edit-${host.id}` opacity=(busy ? 0.64 : 1)
            text "Edit"
          box resize=laidOut width=1 height=1
"##;

#[test]
fn a_row_mounted_dimmed_and_lifted_by_a_resize_round_is_presented_whole() {
    let plan = contract::compile(ROUND).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let save = view(&host, "save").unwrap();
    let batch = host.dispatch_at(save, Event::Press, 100.0);
    let edit = view(&host, "edit-a").unwrap();
    assert!(
        batch.contains(&format!("\"op\":\"create\",\"id\":{edit},")),
        "{batch}"
    );
    assert_eq!(opacity(&batch, edit), Some(1.0), "{batch}");
}
