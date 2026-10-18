//! `share(title=, text=, url=, files=)` (LLP 1069.003; LLP 1125 D12): named
//! arguments lower as `(title, text, url, files)`; the command carries the
//! node whose press ran it (D3); under the agent it is a held request whose
//! answer is a journal line (D2, D6); a file is an `app:/` path the
//! `fs.read` grant covers or an element's `#id`.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::share::{arm, Arm, File, Reach, Share};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
    fn grants(&self) -> &str {
        "fs.read app:/data\n"
    }
}

/// Linux without a shell: no sheet.
const NO_SHEET: Reach = Reach {
    sheet: false,
    files: None,
    pictures: None,
};

fn boot() -> Runner<NoData> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../corpus/share.contract");
    let plan = contract::compile(&std::fs::read_to_string(path).unwrap()).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn view(r: &Runner<NoData>, test_id: &str) -> u32 {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().id
}

fn press(r: &mut Runner<NoData>, test_id: &str) -> u32 {
    let id = view(r, test_id);
    r.dispatch(id, Event::Press).unwrap();
    id
}

#[test]
fn named_arguments_lower_in_the_web_share_order_with_the_pressed_node() {
    let mut r = boot();
    let pressed = press(&mut r, "share-link");
    let c = r.take_commands().pop().unwrap();
    assert_eq!(c.name, "share");
    assert_eq!(
        c.args,
        vec![
            Value::str("A post"),
            Value::Option(None),
            Value::str("https://example.com/post/1"),
            Value::Option(None),
        ]
    );
    assert_eq!(c.source, Some(pressed));
    let pressed = press(&mut r, "share-text");
    let c = r.take_commands().pop().unwrap();
    assert_eq!(c.args[1], Value::str("hello from exact2"));
    assert_eq!(
        (c.args[2].clone(), c.source),
        (Value::Option(None), Some(pressed))
    );
    press(&mut r, "share-card");
    let c = r.take_commands().pop().unwrap();
    assert_eq!(
        c.args[3],
        Value::list(vec![Value::str("#card"), Value::str("app:/data/notes.txt")])
    );
}

#[test]
fn every_host_arm_refuses_holds_or_presents_by_one_rule() {
    let mut r = boot();
    press(&mut r, "share-relative");
    let c = r.take_commands().pop().unwrap();
    let refused = arm(
        &mut r,
        Share::from_args(&c.args),
        c.source,
        false,
        Reach::APPLE,
    );
    assert!(matches!(refused, Arm::Refused(_)), "{refused:?}");
    let pressed = press(&mut r, "share-link");
    let c = r.take_commands().pop().unwrap();
    let none = arm(&mut r, Share::from_args(&c.args), c.source, false, NO_SHEET);
    assert_eq!(none, Arm::Refused("unavailable".into()));
    assert!(matches!(
        arm(
            &mut r,
            Share::from_args(&c.args),
            c.source,
            false,
            Reach::APPLE
        ),
        Arm::Present(_, _)
    ));
    let Arm::Held(ticket) = arm(&mut r, Share::from_args(&c.args), c.source, true, NO_SHEET) else {
        panic!("the agent holds a share on every host");
    };
    let hold = &r.device_holds()[0];
    assert_eq!(
        hold.args,
        format!(
            r#"{{"title":"A post","text":null,"url":"https://example.com/post/1","files":[],"anchor":{pressed}}}"#
        )
    );
    let reply = exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"shared"}}"#),
    )
    .unwrap()
    .0;
    assert!(reply.contains(r#""delivery":"substituted""#), "{reply}");
    let logs = exact_runner::agent::logs(&r, 0);
    for line in [
        "share: refused: url is not an absolute",
        "share: refused: unavailable",
        "device share",
        "share: shared",
    ] {
        assert!(logs.contains(line), "{line} in {logs}");
    }
    // The menu row closes the menu with its press; the hold outlives it.
    press(&mut r, "menu-share");
    let c = r.take_commands().pop().unwrap();
    let Arm::Held(ticket) = arm(&mut r, Share::from_args(&c.args), c.source, true, NO_SHEET) else {
        panic!("held");
    };
    assert_eq!(r.device_holds().len(), 1);
    exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"cancel"}}"#),
    )
    .unwrap();
    assert!(exact_runner::agent::logs(&r, 0).contains("share: dismissed"));
}

/// LLP 1125 D12: a granted app file and the card's picture present on
/// Apple, in order, the picture by its view; the web refuses the picture by
/// name; a file outside `fs.read` is refused everywhere; the agent's hold
/// records the entries and draws nothing.
#[test]
fn files_are_granted_paths_and_pictures_of_elements() {
    let mut r = boot();
    let card = view(&r, "card");
    press(&mut r, "share-card");
    let c = r.take_commands().pop().unwrap();
    let Arm::Present(share, files) = arm(
        &mut r,
        Share::from_args(&c.args),
        c.source,
        false,
        Reach::APPLE,
    ) else {
        panic!("Apple presents files and pictures");
    };
    assert_eq!(share.text.as_deref(), Some("A card"));
    assert_eq!(
        files,
        [
            File::Picture("card".into(), card),
            File::Path("app:/data/notes.txt".into())
        ]
    );
    assert_eq!(
        arm(
            &mut r,
            Share::from_args(&c.args),
            c.source,
            false,
            Reach::WEB
        ),
        Arm::Refused("a picture of an element is not available on the web".into())
    );
    // The web refuses a picture under the agent too: it never draws one.
    assert!(matches!(
        arm(
            &mut r,
            Share::from_args(&c.args),
            c.source,
            true,
            Reach::WEB
        ),
        Arm::Refused(_)
    ));
    let pressed = view(&r, "share-card");
    let Arm::Held(ticket) = arm(
        &mut r,
        Share::from_args(&c.args),
        c.source,
        true,
        Reach::APPLE,
    ) else {
        panic!("held");
    };
    assert_eq!(
        r.device_holds()[0].args,
        format!(
            r##"{{"title":null,"text":"A card","url":null,"files":["#card","app:/data/notes.txt"],"anchor":{pressed}}}"##
        )
    );
    exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"cancel"}}"#),
    )
    .unwrap();
    press(&mut r, "share-file");
    let c = r.take_commands().pop().unwrap();
    assert!(matches!(
        arm(&mut r, Share::from_args(&c.args), c.source, false, Reach::WEB),
        Arm::Present(_, ref f) if f == &[File::Path("app:/data/notes.txt".into())]
    ));
    press(&mut r, "share-ungranted");
    let c = r.take_commands().pop().unwrap();
    for agent in [false, true] {
        assert_eq!(
            arm(
                &mut r,
                Share::from_args(&c.args),
                c.source,
                agent,
                Reach::APPLE
            ),
            Arm::Refused("app:/cache/notes.txt is outside the app's fs.read grants".into())
        );
    }
    // The JSON door: the files, each by path or by view.
    let reply = exact_runner::share::request(
        &mut r,
        r##"{"command":"share","text":"A card","files":["#card","app:/data/notes.txt"],"agent":false}"##,
    );
    assert_eq!(
        reply,
        format!(
            r#"{{"present":true,"files":[{{"id":"card","view":{card}}},{{"path":"app:/data/notes.txt"}}]}}"#
        )
    );
    // Shown under the agent (`--touch platform`): held and presented both;
    // the host's answer with the sheet's outcome is the journal's line.
    let shown = exact_runner::share::request(
        &mut r,
        r##"{"command":"share","text":"A card","files":["#card"],"agent":true,"present":true}"##,
    );
    let ticket = r.device_holds().last().unwrap().ticket;
    assert_eq!(
        shown,
        format!(r#"{{"ticket":{ticket},"present":true,"files":[{{"id":"card","view":{card}}}]}}"#)
    );
    exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"tap","ticket":{ticket},"choice":"cancel"}}"#),
    )
    .unwrap();
    assert!(r.device_holds().iter().all(|h| h.ticket != ticket));
    assert!(exact_runner::agent::logs(&r, 0).contains("share: dismissed"));
    // The document pickers' door takes the same flag (LLP 1069.010): held
    // and presented, the person's choice answers the hold as `type @t` does.
    let picker = exact_runner::file_pickers::request(
        &mut r,
        r#"{"command":"showDirectoryPicker","id":"card","agent":true,"present":true}"#,
    );
    let ticket = r.device_holds().last().unwrap().ticket;
    assert_eq!(
        picker,
        format!(r#"{{"ticket":{ticket},"present":true,"view":{card},"id":"card"}}"#)
    );
    exact_runner::agent::answer(
        &mut r,
        &format!(r#"{{"op":"type","ticket":{ticket},"text":"/tmp/chosen"}}"#),
    )
    .unwrap();
    assert!(r.device_holds().iter().all(|h| h.ticket != ticket));
    let missing = exact_runner::share::request(&mut r, r##"{"files":["#gone"],"web":false}"##);
    assert_eq!(missing, r#"{"refused":"no element with id \"gone\""}"#);
    let web = exact_runner::share::request(&mut r, r##"{"files":["#card"],"web":true}"##);
    assert!(web.contains("not available on the web"), "{web}");
    let logs = exact_runner::agent::logs(&r, 0);
    for line in [
        "share: refused: a picture of an element is not available on the web",
        "share: refused: app:/cache/notes.txt is outside the app's fs.read grants",
        "share: refused: no element with id",
    ] {
        assert!(logs.contains(line), "{line} in {logs}");
    }
}

/// A literal `#id` no element carries is refused when it compiles; one an
/// element with a computed id might carry is left to the runner.
#[test]
fn a_literal_picture_of_no_element_is_refused_at_compile_time() {
    let missing = "component A\n  action go\n    share(files=[\"#nope\"])\n  view\n    button press=go id=\"card\"\n      text \"a\"\n";
    let e = contract::compile(missing).unwrap_err();
    assert!(format!("{e:?}").contains("lower-share-picture"), "{e:?}");
    let found = missing.replace("#nope", "#card");
    assert!(contract::compile(&found).is_ok());
    let computed = "component A\n  state n = \"x\"\n  action go\n    share(files=[\"#nope\"])\n  view\n    button press=go id=n\n      text \"a\"\n";
    assert!(contract::compile(computed).is_ok());
}
