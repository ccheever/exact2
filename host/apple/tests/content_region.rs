//! Real Host first-layout cut, stale completion and native artifact ownership.
use exact_apple::{content_region::ContentRegionRegistration, Host};
use exact_kernel::{MonospaceMeasurer, TextMeasureRequest, TextMeasurer, TextMetrics};
use exact_runner::{DataError, DataSource, Event, Value};
use std::{cell::Cell, rc::Rc};

struct Data;
impl DataSource for Data {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        if name == "blob" {
            Ok(Value::Record(
                vec![Value::Str("giant α body ".repeat(8192).into())].into(),
            ))
        } else {
            Err(DataError::UnknownSource(name.into()))
        }
    }
}
struct ShellOnly(Rc<Cell<usize>>);
impl TextMeasurer for ShellOnly {
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if request.runs.iter().map(|r| r.text.len()).sum::<usize>() > 1024 {
            self.0.set(self.0.get() + 1);
            panic!("registered giant escaped into foreign measurer");
        }
        MonospaceMeasurer::default().measure(request)
    }
}
fn registration() -> ContentRegionRegistration {
    ContentRegionRegistration {
        activate: None,
        owner: "owner",
        content: "content",
        pending: "pending",
    }
}
type Fixture = (Host<Data>, String, Rc<Cell<usize>>);
fn fixture(ids: ContentRegionRegistration) -> Result<Fixture, exact_apple::HostError> {
    let source = r#"shape Blob
  text: string

component App
  state draft = ""
  action edit(value: string) writes draft
    draft = value
  resource doc = blob() as shape Blob
  view
    column width="100%" height="100%"
      view id="owner" width="100%" height=400 flex-shrink=0 overflow-x="hidden" overflow-y="hidden"
        view id="content" width="100%" height="100%"
          scroll testId="document" width="100%" height="100%"
            text doc.text font-size=16
        text "Preparing exact content…" id="pending"
      input value=draft change=edit testId="input"
      text draft testId="echo"
"#;
    let plan = contract::compile(source).unwrap().encode();
    let count = Rc::new(Cell::new(0));
    Host::boot_region(
        &plan,
        Data,
        Box::new(ShellOnly(count.clone())),
        600.,
        800.,
        ids,
    )
    .map(|(host, batch)| (host, batch, count))
}
struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn settle(h: &mut Host<Data>, drops: &Rc<Cell<usize>>) -> usize {
    let mut turns = 0;
    while let Some((id, request)) = h.pending_region_request() {
        let metrics = request.with_request(|r| MonospaceMeasurer::default().measure(r));
        let batch = h.complete_region_text(id, metrics, Rc::new(DropProbe(drops.clone())));
        assert!(!batch.contains("\"error\":\""), "{batch}");
        turns += 1;
        assert!(turns <= 64, "unbounded offer discovery");
    }
    turns
}
#[test]
fn first_layout_uses_real_pending_and_never_calls_foreign_giant() {
    let (mut h, batch, calls) = fixture(registration()).unwrap();
    assert!(batch.contains("\"op\":\"region\""), "{batch}");
    assert!(batch.contains("\"selection\":\"pending\""), "{batch}");
    let (id, _) = h.pending_region_request().unwrap();
    let snapshot = h.region_request_json(id).unwrap();
    assert!(snapshot.contains("giant α body"));
    let input = h.runner().kernel().find_by_test_id("input")[0];
    let view = h.runner().kernel().node_by_key(input).unwrap().id;
    let typed = h.dispatch_at(view, Event::Change("Aα exact input".into()), 10.);
    assert!(typed.contains("Aα exact input"));
    assert_eq!(
        h.pending_region_request().unwrap().0,
        id,
        "unrelated typing must keep request identity"
    );
    h.resize(601., 800.);
    assert_eq!(calls.get(), 0);
}
#[test]
fn stale_bad_metrics_drop_only_their_owner_and_cannot_publish() {
    let (mut h, _, _) = fixture(registration()).unwrap();
    let old = h.pending_region_request().unwrap().0;
    h.resize(611., 800.);
    let current = h.pending_region_request().unwrap().0;
    assert_ne!(old, current);
    let drops = Rc::new(Cell::new(0));
    let before = h.engine().now();
    let batch = h.complete_region_text(
        old,
        TextMetrics {
            width: f32::NAN,
            height: -1.,
            first_baseline: None,
        },
        Rc::new(DropProbe(drops.clone())),
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert_eq!(drops.get(), 1);
    assert_eq!(h.engine().now(), before);
    assert_eq!(h.pending_region_request().unwrap().0, current);
    assert!(settle(&mut h, &drops) > 0);
    let batch = h.resize(611., 800.);
    assert!(batch.contains("\"selection\":\"accepted\""), "{batch}");
    let prior = drops.get();
    drop(h);
    assert!(
        drops.get() > prior,
        "native artifact survives until accepted publication drops"
    );
}
#[test]
fn accepted_source_and_extent_survive_pending_width_replacement() {
    let (mut h, _, calls) = fixture(registration()).unwrap();
    let drops = Rc::new(Cell::new(0));
    settle(&mut h, &drops);
    let old = h.region_publication_id().unwrap();
    let before = drops.get();
    let batch = h.resize(450., 800.);
    assert!(batch.contains("\"current\":false"), "{batch}");
    assert_eq!(h.region_publication_id(), Some(old));
    assert_eq!(
        drops.get(),
        before,
        "old accepted artifact must remain pinned"
    );
    settle(&mut h, &drops);
    assert_ne!(h.region_publication_id(), Some(old));
    assert_eq!(calls.get(), 0);
}
#[test]
fn invalid_registration_refuses_before_any_layout() {
    let mut ids = registration();
    ids.owner = "absent";
    assert!(fixture(ids).is_err());
}

static RELEASES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
extern "C" fn release(_: *mut std::ffi::c_void) {
    RELEASES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
#[test]
fn bridge_completion_owns_payload_even_when_runtime_not_booted() {
    let mut bridge = exact_apple::abi::Bridge::<Data>::new();
    RELEASES.store(0, std::sync::atomic::Ordering::SeqCst);
    let owner = exact_apple::content_region::NativeRegionOwner::new(std::ptr::null_mut(), release);
    let n = bridge.region_complete(
        99,
        exact_apple::measure::CMetrics {
            width: f32::NAN,
            height: -1.,
            baseline: -1.,
        },
        Rc::new(owner),
    );
    assert!(String::from_utf8_lossy(bridge.output_bytes(n as usize)).contains("not booted"));
    assert_eq!(RELEASES.load(std::sync::atomic::Ordering::SeqCst), 1);
}
