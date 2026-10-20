//! Facts a development host holds over the app's own (LLP 1122 T7): the runner's `hold_facts`.
use super::tamber::{app, booted, parsed, press, row, MODES};
use exact_kernel::StyleId;
use exact_plan::Value;

/// In a copy the design tool pins (LLP 1122 T7, theme2's F2 ruling) the
/// host holds the scheme, a pinned axis and the root size over the app's
/// own: the app's `setMode`, `setScheme` and `setRootFontSize` are kept as
/// asked and not applied (no command reaches the host), and handed back
/// each is the app's latest choice, in one commit.
#[test]
fn held_facts_keep_the_apps_own_choices_until_handed_back() {
    let path = app(
        "held-facts",
        &[
            ("app.tamber", MODES),
            ("app.contract", "component App\n  action dim\n    setMode(\"dark-style\", \"dim\")\n  action dark\n    setScheme(\"dark\")\n  action big\n    setRootFontSize(24)\n  view\n    column testId=\"page\" background-color=\"page\" color=\"ink\"\n      button press=dim testId=\"dim\"\n        text \"dim\"\n      button press=dark testId=\"dark\"\n        text \"dark\"\n      button press=big testId=\"big\"\n        text \"big\"\n"),
        ],
    );
    let mut r = booted(contract::compile_path(&path).unwrap());
    let (black, dim) = (
        parsed("light-dark(#ffffff, #000000)"),
        parsed("light-dark(#ffffff, #161e27)"),
    );
    // Refused: an axis value the design lacks.
    assert!(r
        .hold_facts(Some((true, &[("dark-style", "grey")], true)))
        .is_err());
    assert!(r
        .hold_facts(Some((true, &[("dark-style", "black")], true)))
        .unwrap()
        .is_none());
    r.take_commands();
    press(&mut r, "dim");
    press(&mut r, "dark");
    press(&mut r, "big");
    assert_eq!(
        row(&r, "page", StyleId::BackgroundColor),
        black,
        "the pinned axis holds"
    );
    assert_eq!(r.root_font_size(), 16.0, "the root size holds");
    assert!(
        r.take_commands()
            .iter()
            .all(|c| c.name != "setScheme" && c.name != "setRootFontSize"),
        "no held command reaches the host"
    );
    let asked = r.asked().clone();
    assert_eq!(
        asked.modes.get("dark-style").map(String::as_str),
        Some("dim")
    );
    assert_eq!(asked.scheme, Some(Value::str("dark")));
    assert_eq!(asked.root, Some(Value::Number(24.0)));
    // Handed back: the app's latest choice of each, in one commit.
    assert!(r.hold_facts(None).unwrap().is_some());
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), dim);
    assert_eq!(r.root_font_size(), 24.0);
    let names: Vec<String> = r.take_commands().into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["setScheme", "setRootFontSize"]);
    // Not held, the app's own applies at once, and is still kept as asked.
    assert!(r.set_mode("dark-style", "black").unwrap().is_some());
    press(&mut r, "dim");
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), dim);
    assert!(
        r.hold_facts(None).unwrap().is_none(),
        "nothing held: nothing handed back"
    );
    // Taking over the root size the app set lays out at the host's at once.
    press(&mut r, "big");
    assert_eq!(r.root_font_size(), 24.0);
    assert!(r.hold_facts(Some((false, &[], true))).unwrap().is_some());
    assert_eq!(r.root_font_size(), 16.0, "the app's own size set aside");
    // A pin that holds less: what it no longer holds takes the app's latest choice.
    r.hold_facts(Some((true, &[("dark-style", "black")], true)))
        .unwrap();
    r.set_mode("dark-style", "black").unwrap();
    press(&mut r, "dim");
    assert_eq!(row(&r, "page", StyleId::BackgroundColor), black);
    assert!(r.hold_facts(Some((true, &[], true))).unwrap().is_some());
    assert_eq!(
        row(&r, "page", StyleId::BackgroundColor),
        dim,
        "the axis released is the app's dim"
    );
    assert_eq!(r.root_font_size(), 16.0, "the root size still held");
    r.hold_facts(None).unwrap();
    assert_eq!(r.root_font_size(), 24.0, "handed back: the app's 24");
}
