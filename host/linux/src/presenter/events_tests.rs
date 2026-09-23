//! The Linux host's input and paint against the web's rules: the events
//! beyond press and change reach their handlers, a transparent parent keeps
//! its children's hit boxes, a password paints bullets, and the store's write
//! log does not outlive its commit.
use super::*;
use exact_runner::{Answer, DataError, Store, Value};

#[derive(Default)]
struct Keeps;
impl DataSource for Keeps {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
    fn grants(&self) -> &'static str {
        "secret.keep token\n"
    }
    fn answer(&mut self, store: &mut Store, name: &str, _: &[Value]) -> Result<Answer, DataError> {
        assert_eq!(name, "keep");
        store.set("token", "a secret").unwrap();
        Ok(Answer::Now(Value::Bool(true)))
    }
}

const APP: &str = r#"component App
  state text = ""
  state focuses = 0
  state blurs = 0
  state submits = 0
  state lastKey = ""
  state presses = 0
  mutation kept as shape bool
  action edit(value) writes text
    text = value
  action focused writes focuses
    focuses = focuses + 1
  action blurred writes blurs
    blurs = blurs + 1
  action sent writes submits
    submits = submits + 1
  action keyed(value) writes lastKey
    lastKey = value
  action pressed writes presses
    presses = presses + 1
  action keep writes kept
    send kept = keep()
  view
    column width=400 height=400
      input value=text change=edit submit=sent focus=focused blur=blurred key=keyed testId="field" height=32
      button "Other" press=pressed testId="other" height=32
      box opacity=0 width=200 height=40
        button "Ghost" press=pressed testId="ghost" width=200 height=40
      button "Keep" press=keep testId="keep" height=32
      input value="abc" type="password" testId="secret" width=200 height=32
      input value="•••" testId="shown" width=200 height=32
      text `${focuses} ${blurs} ${submits} ${lastKey} ${presses}` testId="log" height=20
"#;

fn boot() -> Presenter<Keeps> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Keeps,
        (400., 400.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}
fn id<D: DataSource>(p: &Presenter<D>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}
fn log<D: DataSource>(p: &Presenter<D>) -> String {
    let k = p.host().kernel();
    let node = k.node_by_key(k.find_by_test_id("log")[0]).unwrap();
    node.props.str(PropId::Text).unwrap().to_string()
}

#[test]
fn keys_submit_focus_and_blur_reach_their_handlers() {
    let mut p = boot();
    let field = id(&p, "field");
    p.type_key(field, "KeyA", "a", true, false).unwrap();
    assert_eq!(
        log(&p),
        "1 0 0 a 0",
        "focus, then the key, then the typed character"
    );
    p.type_key(field, "Enter", "Enter", true, false).unwrap();
    assert_eq!(
        log(&p),
        "1 0 1 Enter 0",
        "Enter at a single-line input submits it"
    );
    let k = p.host().kernel();
    let value = k
        .node(field)
        .unwrap()
        .props
        .str(PropId::Value)
        .unwrap()
        .to_string();
    assert_eq!(value, "a", "Enter is not text in a single-line input");
    p.tap(id(&p, "other")).unwrap();
    assert_eq!(
        log(&p),
        "1 1 1 Enter 1",
        "a press elsewhere blurs the field"
    );
}

#[test]
fn a_transparent_parent_keeps_its_childs_hit_box() {
    let mut p = boot();
    p.tap(id(&p, "ghost")).unwrap();
    assert!(
        log(&p).ends_with(" 1"),
        "opacity is paint only: {}",
        log(&p)
    );
}

#[test]
fn a_password_paints_one_bullet_a_character() {
    let mut p = boot();
    let pixmap = p.frame();
    let rect = |p: &mut Presenter<Keeps>, test_id: &str| p.rect_of(id(p, test_id)).unwrap();
    let (secret, shown) = (rect(&mut p, "secret"), rect(&mut p, "shown"));
    assert_eq!((secret.0, secret.2, secret.3), (shown.0, shown.2, shown.3));
    let row = |y: f32, dy: usize| {
        let y = y as usize + dy;
        (secret.0 as usize..(secret.0 + secret.2) as usize)
            .map(|x| pixmap.pixel(x as u32, y as u32).unwrap().red())
            .collect::<Vec<_>>()
    };
    let inked = (0..secret.3 as usize)
        .filter(|&dy| row(secret.1, dy).iter().any(|&r| r < 200))
        .count();
    assert!(inked > 0, "the password paints something");
    for dy in 0..secret.3 as usize {
        assert_eq!(
            row(secret.1, dy),
            row(shown.1, dy),
            "row {dy}: \"abc\" masked is \"•••\""
        );
    }
}

#[test]
fn store_writes_are_dropped_with_their_commit() {
    let mut p = boot();
    p.tap(id(&p, "keep")).unwrap();
    let kept = |p: &Presenter<Keeps>| {
        p.host()
            .runner()
            .store_names()
            .contains(&"token".to_string())
    };
    for _ in 0..100 {
        if kept(&p) {
            break;
        }
        p.pump(p.host().now());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(kept(&p), "the memory store keeps the value");
    assert!(
        p.host.take_store_writes_for_test().is_empty(),
        "the write log went with its commit"
    );
}
