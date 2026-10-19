//! CSS `visibility` on the Linux host: a hidden box keeps its geometry and
//! paints, hits and focuses nothing of its own. A descendant that computes
//! `visible` still paints and is hit. One that inherits `hidden` does not.
use super::*;

const APP: &str = r##"component App
  view
    column width=200 height=200 background-color="#ffffff"
      box testId="parent" width=120 height=100 background-color="#ff0000" visibility="hidden" position="relative"
        box testId="shown" width=40 height=30 background-color="#00ff00" visibility="visible" tabindex=0 position="absolute" left=8 top=8
        box testId="kept" width=40 height=30 background-color="#0000ff" tabindex=0 position="absolute" left=72 top=8
"##;

fn boot() -> Presenter<()> {
    let (p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (200., 200.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn id(p: &Presenter<()>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn rgba(p: &mut Presenter<()>, x: f32, y: f32) -> [u8; 4] {
    let c = p.frame().pixel(x as u32, y as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn holds(rect: Rect4, point: (f32, f32)) -> bool {
    point.0 >= rect.0 && point.1 >= rect.1 && point.0 < rect.0 + rect.2 && point.1 < rect.1 + rect.3
}

#[test]
fn a_visible_descendant_of_a_hidden_box_paints_and_is_hit() {
    let mut p = boot();
    let (parent, shown, kept) = (id(&p, "parent"), id(&p, "shown"), id(&p, "kept"));
    let parent_rect = p.rect_of(parent).unwrap();
    let shown_rect = p.rect_of(shown).unwrap();
    let kept_rect = p.rect_of(kept).unwrap();
    assert!(
        parent_rect.2 > 0. && parent_rect.3 > 0.,
        "the hidden box keeps its geometry"
    );
    assert!(shown_rect.2 > 0. && kept_rect.2 > 0.);
    let mid = |r: Rect4| (r.0 + r.2 / 2., r.1 + r.3 / 2.);
    let (sx, sy) = mid(shown_rect);
    let (kx, ky) = mid(kept_rect);
    // Below both absolutely placed children, still inside the parent.
    let probe = (parent_rect.0 + 4., parent_rect.1 + parent_rect.3 - 4.);
    assert!(
        holds(parent_rect, probe) && !holds(shown_rect, probe) && !holds(kept_rect, probe),
        "the probe must be the parent's own area: {probe:?} shown {shown_rect:?} kept {kept_rect:?}"
    );
    assert_eq!(
        rgba(&mut p, sx, sy),
        [0, 255, 0, 255],
        "the visible child paints"
    );
    let kept_px = rgba(&mut p, kx, ky);
    assert_ne!(
        kept_px,
        [0, 0, 255, 255],
        "an inheriting child paints nothing: {kept_px:?}"
    );
    let own = rgba(&mut p, probe.0, probe.1);
    assert_ne!(
        own,
        [255, 0, 0, 255],
        "the hidden box paints nothing of its own: {own:?}"
    );
    assert_eq!(p.hit(sx, sy), Some(shown));
    let at_kept = p.hit(kx, ky);
    assert_ne!(at_kept, Some(kept));
    assert_ne!(at_kept, Some(parent));
    let at_own = p.hit(probe.0, probe.1);
    assert_ne!(at_own, Some(parent));
    assert!(p.focusable(shown), "a visible tabindex box takes the focus");
    assert!(!p.focusable(kept), "a hidden box is not focusable");
}

#[test]
fn a_visible_descendant_presses_its_hidden_ancestor_with_feedback() {
    for child in ["box tabindex=0", "input value=\"abcd\""] {
        for hidden_on_down in [false, true] {
            for feedback in ["", "-exact-press-scale=0.9"] {
                let source = format!(
                    r#"component App
  state hidden = {initially_hidden}
  state presses = 0
  action down
    hidden = true
  action pressed
    presses = presses + 1
  view
    column width=300 height=300
      input testId="other" value="other" width=180 height=40
      box visibility=(hidden ? "hidden" : "visible") width=200 height=150 press=pressed pointerdown=down {feedback}
        column
          {child} testId="child" visibility="visible" width=180 height=120
      text presses.toString() testId="presses"
"#,
                    initially_hidden = !hidden_on_down
                );
                let (mut p, error) = Presenter::boot_with(
                    &crate::test_plan(&source),
                    (),
                    (300., 300.),
                    1.,
                    PathBuf::new(),
                    PainterChoice::Cpu,
                )
                .unwrap();
                assert!(error.is_none(), "{error:?}");
                let child_id = id(&p, "child");
                p.set_focus(Some(id(&p, "other")), 0.);
                let (x, y, _, _) = p.rect_of(child_id).unwrap();
                assert_eq!(p.hit(x + 20., y + 20.), Some(child_id));
                p.pointer_down(x + 20., y + 20., 1.).unwrap();
                p.pointer_up(x + 20., y + 20., 2.).unwrap();
                assert_eq!(
                    p.focus(),
                    Some(child_id),
                    "{child} {hidden_on_down} {feedback}"
                );
                assert_eq!(
                    p.host
                        .kernel()
                        .node(id(&p, "presses"))
                        .unwrap()
                        .props
                        .str(PropId::Text),
                    Some("1"),
                    "{child} {hidden_on_down} {feedback}"
                );
            }
        }
    }
}

#[test]
fn mouse_click_uses_the_common_ancestor_of_down_and_release() {
    common_ancestor_clicks(false);
}

#[test]
fn mouse_click_common_ancestor_with_an_acknowledged_display() {
    common_ancestor_clicks(true);
}

fn common_ancestor_clicks(attached: bool) {
    // Chrome 151, Input.dispatchMouseEvent: removal suppresses click; hiding
    // does not. Pointerup's style changes happen after click target selection.
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for direct in [false, true]
        .into_iter()
        .filter(|direct| !attached || !direct)
    {
        for feedback in ["", "-exact-press-scale=0.9"] {
            for (down, up, release, press) in [
                ("fieldDisplay = \"none\"", "", (20., 20.), "parent"),
                ("fieldVisibility = \"hidden\"", "", (20., 20.), "parent"),
                ("parentDisplay = \"none\"", "", (20., 20.), "root"),
                ("parentVisibility = \"hidden\"", "", (20., 20.), "root"),
                ("fieldShown = false", "", (20., 20.), ""),
                ("parentShown = false", "", (20., 20.), ""),
                ("", "", (140., 20.), "parent"),
                ("", "", (250., 160.), "root"),
                ("", "fieldDisplay = \"none\"", (20., 20.), "field"),
                ("", "fieldVisibility = \"hidden\"", (20., 20.), "field"),
                ("", "parentDisplay = \"none\"", (20., 20.), "field"),
                ("", "fieldDisplay = \"none\"", (140., 20.), "parent"),
            ] {
                let source = format!(
                    r#"component App
  state fieldDisplay = "block"
  state fieldVisibility = "inherit"
  state parentDisplay = "block"
  state parentVisibility = "visible"
  state fieldShown = true
  state parentShown = true
  state seen = ""
  action down
    {down}
  action up
    {up}
  action pressed(name: string)
    seen = name
  view
    box width=300 height=200 press=pressed("root")
      if parentShown
        box width=220 height=100 display=parentDisplay visibility=parentVisibility press=pressed("parent")
          if fieldShown
            input testId="field" width=100 height=60 display=fieldDisplay visibility=fieldVisibility pointerdown=down pointerup=up press=pressed("field") {feedback}
          input testId="sibling" width=100 height=60 position="absolute" left=120 top=0 press=pressed("sibling")
      text seen testId="seen"
"#,
                    down = if down.is_empty() { "seen = seen" } else { down },
                    up = if up.is_empty() { "seen = seen" } else { up },
                );
                let (mut p, error) = Presenter::boot_with(
                    &crate::test_plan(&source),
                    (),
                    (300., 200.),
                    1.,
                    PathBuf::new(),
                    PainterChoice::Cpu,
                )
                .unwrap();
                assert!(error.is_none(), "{error:?}");
                if attached {
                    let frame = p.display_frame().unwrap();
                    assert!(p.display_complete(&frame));
                }
                assert_eq!(p.hit(20., 20.), Some(id(&p, "field")));
                if direct {
                    p.set_pointer_kind(PointerKind::Touch);
                }
                p.pointer_down(20., 20., 1.).unwrap();
                p.pointer_move(release.0, release.1, 2.).unwrap();
                p.pointer_up(release.0, release.1, 3.).unwrap();
                let label =
                    format!("direct={direct} {feedback} down={down} up={up} release={release:?}");
                actual.push((
                    label.clone(),
                    p.host
                        .kernel()
                        .node(id(&p, "seen"))
                        .unwrap()
                        .props
                        .str(PropId::Text)
                        .unwrap()
                        .to_string(),
                ));
                expected.push((label, if direct { "" } else { press }.to_string()));
            }
        }
    }
    assert_eq!(actual, expected);
}
