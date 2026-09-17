//! Real Contract buttons drive the same logical model across all three modes.
use exact_kernel::{Kernel, NodeType, PropId};
use exact_plan::Value;
use exact_runner::{Event, Runner};
use interaction_gallery_data::Gallery;

fn boot() -> Runner<Gallery> {
    let plan = contract::compile(include_str!("../../app.contract")).unwrap();
    let baked = contract::bake(plan, Gallery::default()).unwrap();
    let mut runner = Runner::boot(
        baked,
        Gallery::default(),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    runner
        .act("chooseRendering", vec![Value::str("manual")])
        .unwrap();
    runner
}

fn press(r: &mut Runner<Gallery>, name: &str) {
    let ids = r.kernel().find_by_test_id(name);
    assert_eq!(ids.len(), 1, "missing or ambiguous {name}");
    let id = r.kernel().node_by_key(ids[0]).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
}

fn present(r: &Runner<Gallery>, name: &str) -> bool {
    r.kernel().find_by_test_id(name).len() == 1
}

fn assert_focus_target(r: &mut Runner<Gallery>, focus: &str) {
    let commands = r.take_commands();
    assert!(
        commands
            .iter()
            .any(|c| c.name == "focus" && c.args == vec![Value::str(focus)]),
        "missing focus command for {focus}: {commands:?}"
    );
    let keys = r.kernel().find_by_test_id(focus);
    assert_eq!(keys.len(), 1, "focus target not uniquely mounted: {focus}");
    let node = r.kernel().node_by_key(keys[0]).unwrap();
    assert_eq!(node.node_type, NodeType::Pressable);
    assert_eq!(
        node.props.str(PropId::Id),
        Some(focus),
        "focus resolves authored id, not testId: {focus}"
    );
    assert_ne!(node.props.bool(PropId::Disabled), Some(true));
    let mut pending = r.roots();
    let mut authored_matches = 0;
    while let Some(id) = pending.pop() {
        let node = r.kernel().node(id).unwrap();
        authored_matches += usize::from(node.props.str(PropId::Id) == Some(focus));
        pending.extend(node.children());
    }
    assert_eq!(authored_matches, 1, "ambiguous authored focus id: {focus}");
}

#[test]
fn focus_commands_name_controls_in_the_resulting_surface() {
    let mut r = boot();
    for (button, focus) in [
        ("open-photo-00002", "close-viewer"),
        ("close-viewer", "open-photo-00002"),
        ("mode-reorder", "mode-reorder"),
        ("lift-photo-00000", "place"),
        ("cancel", "lift-photo-00000"),
        ("mode-sheet", "mode-sheet"),
        ("mode-photos", "mode-photos"),
        ("open-photo-00002", "close-viewer"),
        ("delete-photo", "mode-photos"),
    ] {
        r.take_commands();
        press(&mut r, button);
        assert_focus_target(&mut r, focus);
    }
}

#[test]
fn return_and_place_focus_the_current_identity_on_its_new_page() {
    let mut r = boot();
    press(&mut r, "next-page");
    press(&mut r, "open-photo-00012");
    press(&mut r, "previous-photo");
    r.take_commands();
    press(&mut r, "close-viewer");
    assert_focus_target(&mut r, "open-photo-00011");
    assert!(!present(&r, "open-photo-00012"));

    press(&mut r, "mode-reorder");
    press(&mut r, "lift-photo-00000");
    press(&mut r, "next-page");
    press(&mut r, "before-photo-00015");
    r.take_commands();
    press(&mut r, "place");
    assert_focus_target(&mut r, "lift-photo-00000");
    assert!(present(&r, "card-photo-00015"));
    assert!(!present(&r, "card-photo-00001"));
}

#[test]
fn cancelling_from_another_page_remounts_the_picked_up_identity_for_focus() {
    let mut r = boot();
    press(&mut r, "mode-reorder");
    press(&mut r, "lift-photo-00000");
    press(&mut r, "next-page");
    assert!(!present(&r, "card-photo-00000"));
    r.take_commands();
    press(&mut r, "cancel");
    assert_focus_target(&mut r, "lift-photo-00000");
    assert!(present(&r, "card-photo-00001"));
    assert!(!present(&r, "card-photo-00012"));
}

#[test]
fn photos_page_open_switch_return_delete_and_preserve_typing() {
    let mut r = boot();
    assert!(present(&r, "open-photo-00000"));
    assert!(present(&r, "open-photo-00011"));
    assert!(!present(&r, "open-photo-00012"));
    r.act(
        "edit",
        vec![Value::str("Keep this note while changing modes")],
    )
    .unwrap();
    press(&mut r, "next-page");
    press(&mut r, "open-photo-00012");
    assert!(present(&r, "viewer-image"));
    assert!(!present(&r, "open-photo-00012"));
    press(&mut r, "previous-photo");
    press(&mut r, "close-viewer");
    assert!(present(&r, "open-photo-00011"));
    press(&mut r, "open-photo-00011");
    press(&mut r, "delete-photo");
    assert!(!present(&r, "viewer-image"));
    assert!(!present(&r, "open-photo-00011"));
    assert_eq!(
        r.slot("draft"),
        Some(&Value::str("Keep this note while changing modes"))
    );
}

#[test]
fn rearranging_requires_explicit_place_and_sheet_uses_same_records() {
    let mut r = boot();
    press(&mut r, "mode-reorder");
    press(&mut r, "lift-photo-00000");
    press(&mut r, "later");
    press(&mut r, "cancel");
    assert!(present(&r, "card-photo-00000"));
    press(&mut r, "lift-photo-00000");
    press(&mut r, "next-page");
    press(&mut r, "before-photo-00015");
    press(&mut r, "place");
    assert!(present(&r, "card-photo-00000"));
    press(&mut r, "mode-sheet");
    assert!(present(&r, "note-photo-00000"));
    press(&mut r, "sheet-peek");
    press(&mut r, "sheet-full");
    press(&mut r, "mode-photos");
    press(&mut r, "open-photo-00000");
    press(&mut r, "close-viewer");
    assert!(present(&r, "open-photo-00000"));
}

#[test]
fn largest_fixture_still_projects_twelve_cards_and_reset_cancels_preview() {
    let mut r = boot();
    press(&mut r, "count-25000");
    assert!(present(&r, "open-photo-00011"));
    assert!(!present(&r, "open-photo-00012"));
    press(&mut r, "mode-reorder");
    press(&mut r, "lift-photo-00000");
    press(&mut r, "reset");
    assert!(!present(&r, "move-preview"));
    press(&mut r, "mode-photos");
    assert!(present(&r, "open-photo-00000"));
}
