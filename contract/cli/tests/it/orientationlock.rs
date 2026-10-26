//! LLP 1133.029 D1–D3: `orientationlock` lowers to a prop on any element; a
//! literal outside its words is refused naming them; other platforms'
//! names point at it; and the bake refuses a lock inside a `dialog` or
//! popover, and a literal type the manifest's `orientation` leaves no room
//! for.

use exact_kernel::{Kernel, PropId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn it_lowers_on_any_element() {
    let src = "component App\n  state tilt = true\n  view\n    column testId=\"box\" orientationlock=\"portrait\"\n      canvas testId=\"world\" orientationlock=(tilt ? \"current\" : \"none\")\n";
    let plan = contract::compile(src).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let prop = |id: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .props
            .str(PropId::Orientationlock)
            .map(str::to_string)
    };
    assert_eq!(prop("box").as_deref(), Some("portrait"));
    assert_eq!(prop("world").as_deref(), Some("current"));
}

#[test]
fn a_literal_outside_the_words_is_refused_and_other_names_point_here() {
    for value in ["sideways", "Portrait", "upside-down", ""] {
        let src = format!("component App\n  view\n    column orientationlock=\"{value}\"\n");
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(
            error.contains("lower-attr-value")
                && error.contains("\"landscape-secondary\"")
                && error.contains("\"current\""),
            "orientationlock={value}: {error}"
        );
    }
    for name in [
        "screenOrientation",
        "lockOrientation",
        "requestedOrientation",
    ] {
        let src = format!("component App\n  view\n    column {name}=\"portrait\"\n");
        let error = contract::compile(&src).unwrap_err().to_string();
        assert!(error.contains("orientationlock"), "{name}: {error}");
    }
}

/// An app directory with `app.json` (its `orientation`, if any) and
/// `app.contract`, compiled by path as a host's bake compiles it.
fn bake(name: &str, orientation: Option<&str>, src: &str) -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "exact-orientationlock-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let orientation = orientation.map_or(String::new(), |o| format!(",\"orientation\":\"{o}\""));
    std::fs::write(
        dir.join("app.json"),
        format!(
            "{{\"name\":\"T\",\"app\":{{\"id\":\"com.example.t\",\"name\":\"T\"}}{orientation}}}"
        ),
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), src).unwrap();
    let result = contract::compile_path(&dir.join("app.contract"));
    let _ = std::fs::remove_dir_all(&dir);
    result
        .map(|_| ())
        .map_err(|e| format!("[{}] {}", e.id, e.message))
}

#[test]
fn the_bake_refuses_a_type_the_manifest_leaves_no_room_for() {
    let lock =
        |word: &str| format!("component App\n  view\n    column orientationlock=\"{word}\"\n");
    let error = bake("outside", Some("portrait"), &lock("landscape")).unwrap_err();
    assert!(
        error.starts_with("[orientationlock-outside-manifest]")
            && error.contains("\"orientation\": \"portrait\"")
            && error.contains("Widen the manifest"),
        "{error}"
    );
    // A narrower type inside the manifest's, `current`, `natural`, `any`,
    // and every word in an app whose manifest says nothing, pass.
    for (manifest, word) in [
        (Some("portrait"), "portrait-primary"),
        (Some("portrait"), "current"),
        (Some("portrait"), "natural"),
        (Some("portrait"), "any"),
        (Some("landscape-primary"), "landscape"),
        (Some("any"), "landscape-secondary"),
        (Some("natural"), "landscape"),
        (None, "portrait-secondary"),
    ] {
        assert_eq!(
            bake("inside", manifest, &lock(word)),
            Ok(()),
            "{manifest:?} {word}"
        );
    }
    // A computed value is the host's to bound.
    let computed =
        "component App\n  state w = \"landscape\"\n  view\n    column orientationlock=w\n";
    assert_eq!(bake("computed", Some("portrait"), computed), Ok(()));
}

#[test]
fn the_bake_refuses_a_lock_inside_a_dialog_or_popover() {
    for (name, src) in [
        ("dialog", "component App\n  view\n    column\n      dialog id=\"d\"\n        column orientationlock=\"landscape\"\n"),
        ("computed", "component App\n  state w = \"landscape\"\n  view\n    column\n      dialog id=\"d\"\n        column orientationlock=w\n"),
        ("popover", "component App\n  view\n    column\n      column id=\"p\" popover=\"auto\"\n        text \"x\" orientationlock=\"portrait\"\n"),
    ] {
        let error = bake(name, None, src).unwrap_err();
        assert!(error.starts_with("[orientationlock-in-top-layer]"), "{name}: {error}");
        let error = contract::compile(src).unwrap_err().to_string();
        assert!(error.contains("orientationlock-in-top-layer"), "{name} (text): {error}");
    }
}
