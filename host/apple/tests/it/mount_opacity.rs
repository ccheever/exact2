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

const SRC: &str = "shape Host
  id: string

component App
  state busy = false
  state hosts: Host[] = []
  action save
    busy = true
    hosts = [Host(id=\"a\")]
  action done
    busy = false
  view
    column
      button press=save testId=\"save\"
        text \"Save\"
      button press=done testId=\"done\"
        text \"Done\"
      for host in hosts key=host.id
        row
          Ghost(label=\"Edit\", disabled=busy, buttonId=`edit-${host.id}`)

component Ghost
  props
    label: string
    disabled: bool
    buttonId: string
  state over = false
  action hover(value: bool)
    over = value
  derive lit = over and not disabled
  view
    button hover=hover disabled=disabled testId=buttonId display=\"flex\" height=\"1.75rem\" border-width=0 background-color=(lit ? \"#f4f4f5\" : \"#00000000\") opacity=(disabled ? 0.64 : 1)
      :active
        background-color=\"#f4f4f5\"
      text label font-size=\"0.875rem\" white-space=\"nowrap\"
";

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
    let edit = view(&host, "edit-a").unwrap();
    assert!(
        mounted.contains(&format!("\"op\":\"create\",\"id\":{edit},")),
        "{mounted}"
    );
    let dimmed = opacity(&mounted, edit).expect("mounted at 0.64");
    assert!((dimmed - 0.64).abs() < 1e-6, "{dimmed}");
    let ended = host.dispatch_at(done, Event::Press, 200.0);
    assert_eq!(view(&host, "edit-a"), Some(edit), "the row stays mounted");
    assert_eq!(opacity(&ended, edit), Some(1.0), "{ended}");
}
