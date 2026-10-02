use super::*;
use exact_runner::Request;
use sha2::{Digest, Sha256};

#[test]
fn surface_work_scope_is_refused_before_presenter_effects() {
    let request = |ticket, mut request: exact_runner::Request| {
        request.grants = Some("surface.read other".into());
        RequestOut {
            ticket,
            target: "save".into(),
            request,
            forced: false,
        }
    };
    let mut surfaces = Surfaces::default();
    surfaces.enqueue(
        request(7, Request::capture_surface("world")),
        "surface.read world\nsurface.read other",
    );
    assert!(surfaces.work.is_empty());
    assert!(matches!(
        surfaces.take_outcomes().as_slice(),
        [(
            7,
            Outcome::Failed {
                kind: FailureKind::Refused,
                ..
            }
        )]
    ));

    let mut allowed = Request::capture_surface("world");
    allowed.grants = Some("surface.read world".into());
    surfaces.enqueue(
        RequestOut {
            ticket: 8,
            target: "save".into(),
            request: allowed,
            forced: false,
        },
        "surface.read world",
    );
    assert_eq!(surfaces.work.len(), 1);
}

#[test]
fn deferred_restore_keeps_bytes_until_commit_and_late_refusal_is_once() {
    for refused in [true, false] {
        let mut c = Canvas {
            id: 1,
            name: "world".into(),
            artifact: String::new(),
            owner: true,
            since: 0,
            held: Default::default(),
            restored_controls: None,
            restore_error: None,
            restore_input: true,
            restore_bytes: Some(vec![1]),
            restore_logged: false,
        };
        c.finish_restore(None, Some(&json!({"world":{"restored":false}})));
        assert_eq!(c.restore_bytes, Some(vec![1]));
        if refused {
            assert!(c
                .finish_restore(Some("restore refused: invalid save".into()), None)
                .unwrap()
                .contains("invalid save"));
            assert!(c
                .finish_restore(Some("restore refused: invalid save".into()), None)
                .is_none());
            assert_eq!(
                c.restore_error.as_deref(),
                Some("restore refused: invalid save")
            );
            assert_eq!(c.restore_bytes, Some(vec![1]));
        } else {
            c.finish_restore(
                None,
                Some(&json!({"world":{"restored":true,"input":{"forwarded":["KeyW"]}}})),
            );
            assert!(c.restore_bytes.is_none());
            assert!(c.held.contains("KeyW"));
        }
        assert!(!c.restore_input);
    }
}
pub(super) fn fixture() -> (PathBuf, Value) {
    let dir = std::env::temp_dir().join(format!(
        "exact-gpu-loader-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("libprobe.{}", std::env::consts::DLL_EXTENSION));
    let source = dir.join("probe.c");
    std::fs::write(
        &source,
        r#"
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
void gpu_load_headless(void) {}
uint32_t gpu_recover(void) { return 0; }
uint32_t gpu_child_view(void) { return 0; }
uint32_t gpu_children_count(void) { return 0; }
uint32_t gpu_children_mode(void) { return 3; }
uint32_t gpu_placement(uint32_t id, uint32_t index, float *h, size_t len) {
  if (len<16) return 0;
  for(int i=0;i<16;i++) h[i]=0;
  h[0]=h[4]=h[8]=h[12]=h[15]=1; h[9]=(float)index;
  return 1;
}
void gpu_unload(void) {}
uint32_t gpu_create_headless(void) { return 1; }
static char bound[4096];
uint32_t gpu_bind_at(uint32_t id, const unsigned char *text, size_t len, double at) {
  if (len >= sizeof(bound)) return 1; memcpy(bound, text, len); bound[len] = 0; return 0;
}
const char *test_bound(void) { return bound; }
static const char reply[] = "{\"hit\":{\"name\":\"cpu\"}}";
uint32_t gpu_agent(uint32_t id, const unsigned char *text, size_t len) {
  for (size_t i=0; i+6<=len; ++i) if (!memcmp(text+i, "layout", 6)) return sizeof(reply)-1;
  return 268435457;
}
static uint32_t cancels = 0;
uint32_t test_cancels(void) { return cancels; }
static char event[2048];
static uint32_t event_id, event_count;
const char *test_input(void) { return event; }
uint32_t test_input_id(void) { return event_id; }
uint32_t test_input_count(void) { return event_count; }
uint32_t gpu_input(uint32_t id, const unsigned char *text, size_t len) {
  if(len>=sizeof(event)) return 1; memcpy(event,text,len);event[len]=0;
  event_id=id; event_count++;
  if (strstr(event,"RejectKey")) return 1;
  if (strstr(event,"cancel") || strstr(event,"blur")) cancels++;
  return strstr(event,"control") && !strstr(event,"jump") ? 1 : 0;
}
uint32_t gpu_wants_input(void) { return 1; }
uint32_t gpu_assets(void) { return 0; }
bool gpu_asset(void) { return true; }
uint32_t gpu_published(void) { return 268435457; }
uint32_t gpu_messages(void) { return 268435457; }
uint32_t gpu_carry(void) { return 268435457; }
uint32_t gpu_restore(void) { return 1; }
void gpu_destroy(void) {}
uint32_t gpu_error(void) { return 0; }
const unsigned char* gpu_out_ptr(void) { return (const unsigned char*)reply; }
"#,
    )
    .unwrap();
    assert!(std::process::Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(&source)
        .arg("-o")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let digest = format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap()));
    let compat = json!({"id":"cohort", "inputs":{"app":"test.app"}, "embedded":{"gpu":{
            "name":path.file_name().unwrap().to_str().unwrap(),"sha256":digest,
            "app":"test.app","cohort":"cohort","trust":"production"}}});
    (path, compat)
}
#[test]
fn module_identity_refuses_stale_foreign_and_missing_bakes_before_loading() {
    let (path, compat) = fixture();
    for (key, value, reason) in [
        ("sha256", "old-product", "digest"),
        ("app", "other.app", "app"),
        ("cohort", "old-cohort", "cohort"),
    ] {
        let mut wrong = compat.clone();
        wrong["embedded"]["gpu"][key] = value.into();
        let error = Abi::open_path(&path, &wrong, "")
            .err()
            .expect("mismatch must refuse");
        assert!(
            error.contains(reason) && error.contains("libprobe"),
            "{error}"
        );
    }
    assert!(Abi::open_path(&path, &Value::Null, "").is_err());
    drop(Abi::open_path(&path, &compat, "").unwrap());
    // A digest-authenticated old child ABI must refuse before any call.
    let source = path.with_file_name("probe.c");
    let old = std::fs::read_to_string(&source)
        .unwrap()
        .replace("gpu_child_view", "gpu_child");
    std::fs::write(&source, old).unwrap();
    assert!(std::process::Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(&source)
        .arg("-o")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let mut old_compat = compat.clone();
    old_compat["embedded"]["gpu"]["sha256"] =
        format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap())).into();
    let error = Abi::open_path(&path, &old_compat, "")
        .err()
        .expect("old ABI refused");
    assert!(error.contains("gpu_child_view"), "{error}");
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn cpu_canvas_pick_is_forwarded_without_a_device() {
    struct NoData;
    impl DataSource for NoData {
        fn query(
            &mut self,
            name: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(name.into()))
        }
    }
    let (path, compat) = fixture();
    let plan = contract::compile("component App\n  view\n    text \"test\"\n").unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 100.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    p.surfaces
        .abis
        .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
    p.surfaces.canvases.insert(
        1,
        Canvas {
            id: 1,
            name: "world".into(),
            artifact: String::new(),
            owner: true,
            since: 0,
            held: BTreeSet::new(),
            restored_controls: None,
            restore_error: None,
            restore_input: false,
            restore_bytes: None,
            restore_logged: false,
        },
    );
    let reply = p.surface_request(1, json!({"op":"layout","x":50,"y":50}));
    assert_eq!(reply["hit"]["name"], "cpu");
    drop(p);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn controls_own_release_and_focus_and_placed_siblings_cover() {
    #[derive(Default)]
    struct NoData;
    impl DataSource for NoData {
        fn query(
            &mut self,
            n: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(n.into()))
        }
    }
    let (path, compat) = fixture();
    let source = r#"component Controls
  state renamed = false
  action rename
    renamed = true
  view
    column
      canvas testId="world" width=100 height=100
        button testId="jump" action=(renamed ? "light" : "jump") width=100 height=100
      button testId="rename" press=rename width=100 height=40
"#;
    let plan = contract::compile(source).unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 200.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    let find = |p: &Presenter<NoData>, name: &str| {
        p.host
            .preorder()
            .into_iter()
            .find(|id| {
                p.host
                    .kernel()
                    .node(*id)
                    .unwrap()
                    .props
                    .str(exact_kernel::PropId::TestId)
                    == Some(name)
            })
            .unwrap()
    };
    let canvas = find(&p, "world");
    let button = find(&p, "jump");
    let rename = find(&p, "rename");
    p.surfaces
        .abis
        .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
    p.surfaces.canvases.insert(
        canvas,
        Canvas {
            id: 1,
            name: "world".into(),
            artifact: String::new(),
            owner: true,
            since: 0,
            held: Default::default(),
            restored_controls: None,
            restore_error: None,
            restore_input: false,
            restore_bytes: None,
            restore_logged: false,
        },
    );
    p.boxes();
    assert!(p.control_input(button, "down", 20., 30., 1, 0.));
    assert_eq!(p.focus(), Some(button));
    p.hardware_key("Space", "Space", true, false);
    assert_eq!(p.control_bindings[&(canvas, u32::MAX - 1)].name, "jump");
    p.hardware_key("Space", "Space", false, false);
    assert!(!p.control_bindings.contains_key(&(canvas, u32::MAX - 1)));
    // A held pointer owns activation keys even when focus is elsewhere.
    // Release it before testing the raw canvas fallback.
    assert!(p.control_input(button, "up", 20., 30., 1, 0.));
    p.focus = None;
    p.hardware_key("Space", "Space", true, false);
    assert!(p.surfaces.canvases[&canvas].held.contains("Space"));
    p.hardware_key("Space", "Space", false, false);
    assert!(p.surfaces.canvases[&canvas].held.is_empty());
    assert!(p.control_input(button, "down", 20., 30., 1, 0.));
    p.focus = Some(button);
    p.hardware_key("Space", "Space", true, false);
    p.host.dispatch_at(rename, Event::Press, 0.);
    p.after_commit();
    p.hardware_key("Space", "Space", true, false); // hardware autorepeat keeps the original press
    assert_eq!(p.control_bindings[&(canvas, u32::MAX - 1)].name, "jump");
    p.hardware_key("Space", "Space", false, false);
    assert!(
        p.control_input(button, "up", 200., 300., 1, 0.),
        "release must still be jump"
    );
    // Restoration uses the same immutable action even after the binary's HUD changed.
    p.surfaces
        .canvases
        .get_mut(&canvas)
        .unwrap()
        .restored_controls = Some(vec![
        json!({"id":4294967294u32,"action":"jump","position":[0,0]}),
    ]);
    assert!(p.type_key(button, "Space", "Space", false, false).is_ok());
    assert!(!p.control_input(button, "down", 20., 30., 3, 0.));
    assert!(
        !p.control_bindings.contains_key(&(canvas, 3)),
        "a refused press owns nothing"
    );
    drop(p);
    let source = r#"component Placed
  state count = 0
  action press
    count = count + 1
  view
    canvas testId="world" width=100 height=100
      button testId="under" width=100 height=100 press=press
      button testId="cover" width=100 height=100 press=press
"#;
    let plan = contract::compile(source).unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        NoData,
        (100., 100.),
        1.,
        path.parent().unwrap().into(),
    )
    .unwrap();
    let canvas = find(&p, "world");
    let under = find(&p, "under");
    p.surfaces
        .abis
        .insert(String::new(), Abi::open_path(&path, &compat, "").unwrap());
    p.surfaces.canvases.insert(
        canvas,
        Canvas {
            id: 1,
            name: "world".into(),
            artifact: String::new(),
            owner: true,
            since: 0,
            held: Default::default(),
            restored_controls: None,
            restore_error: None,
            restore_input: false,
            restore_bytes: None,
            restore_logged: false,
        },
    );
    p.dirty = true; // The injected canvas changes the next presented frame.
    assert!(p.tap(under).unwrap_err().contains("covered or not hit"));
    assert_eq!(
        p.host.runner().slot("count"),
        Some(&exact_runner::Value::Number(0.))
    );
    drop(p);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
#[test]
fn e11_development_module_requires_its_own_completed_receipt() {
    let dir = std::env::temp_dir().join(format!("e11-gpu-receipt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let product = dir.join("gpu.dylib");
    let receipt = dir.join("gpu.dylib.proof.json");
    std::fs::write(&product, b"module").unwrap();
    // A standalone app without delivery has no trust component in its cohort.
    let mut compat = json!({"id":"cohort","inputs":{"app":"game","trust":null},"embedded":{"gpu":{"app":"game","cohort":"cohort","trust":"development","receipt":true}}});
    assert!(verify_module(&product, &compat, "").is_err());
    use sha2::{Digest, Sha256};
    std::fs::write(
        &receipt,
        json!({"inputs":"a".repeat(64),"artifact":format!("{:x}",Sha256::digest(b"module"))})
            .to_string(),
    )
    .unwrap();
    verify_module(&product, &compat, "").unwrap();
    std::fs::write(&product, b"changed").unwrap();
    assert!(verify_module(&product, &compat, "").is_err());
    std::fs::write(&product, b"module").unwrap();
    compat["inputs"]["trust"] = json!("production");
    assert!(verify_module(&product, &compat, "").is_err());
    compat["inputs"]["trust"] = Value::Null;
    compat["embedded"]["gpu"]["trust"] = json!("production");
    assert!(verify_module(&product, &compat, "").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn production_ignores_module_path_override() {
    let default = std::path::Path::new("baked");
    let mut compat = json!({"embedded":{"gpu":{"trust":"production"}}});
    assert_eq!(
        module_path(default, &compat, Some("override".into())),
        default
    );
    compat["embedded"]["gpu"]["trust"] = "development".into();
    assert_eq!(
        module_path(default, &compat, Some("override".into())),
        PathBuf::from("override")
    );
}
#[test]
fn oversized_module_output_is_a_structured_refusal() {
    let (path, compat) = fixture();
    let abi = Abi::open_path(&path, &compat, "").unwrap();
    let reply = abi.agent(1, &json!({"op":"state"}));
    assert!(reply["error"].as_str().unwrap().contains("256 MiB"));
    for symbol in [b"gpu_carry".as_slice(), b"gpu_published", b"gpu_messages"] {
        assert!(abi.read(symbol, 1).is_none());
        assert!(abi.error().unwrap().contains("256 MiB"));
    }
    assert!(abi.bytes(u32::MAX).is_none());
    assert_eq!(abi.bytes(0), Some(vec![]));
    drop(abi);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
