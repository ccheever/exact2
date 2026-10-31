use super::*;
use exact_runner::Value;

fn id(p: &Presenter<()>, name: &str) -> ViewId {
    let k = p.host.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn boot(source: &str, kind: Kind) -> Presenter<()> {
    let source = match kind {
        Kind::Dialog => source.to_owned(),
        Kind::Alert => source.replace(
            "dialog id=\"box\"",
            "dialog role=\"alertdialog\" aria-label=\"Prompt\" id=\"box\"",
        ),
        Kind::Popover => source
            .replace("command=\"show-modal\"", "command=\"show-popover\"")
            .replace(
                "dialog id=\"box\"",
                "column popover=\"manual\" role=\"dialog\" id=\"box\"",
            ),
    };
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(&source),
        (),
        (480., 400.),
        1.,
        PathBuf::new(),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.serve_overlays();
    p.frame();
    p
}

fn tap(p: &mut Presenter<()>, name: &str) {
    p.tap(id(p, name)).unwrap();
    p.run_commands(|| ());
    p.frame();
}

const RADIOS: &str = r#"component App
  state pageInputs = 0
  state pageChanges = 0
  state choice = ""
  action pageInput(value: string)
    pageInputs = pageInputs + 1
  action pageChange(value: string)
    pageChanges = pageChanges + 1
  action choose(value)
    choice = value
  view
    column width="100%" height="100%"
      input type="radio" name="group" testId="page" input=pageInput change=pageChange width=40 height=30
      button "Open" testId="open" commandfor="box" command="show-modal" height=30
      dialog id="box" testId="box" width=220
        input type="radio" name="group" value="a" testId="a" autofocus=true change=choose width=40 height=30
        input type="radio" name="group" disabled=true testId="disabled" width=40 height=30
        input type="radio" name="group" value="b" testId="b" change=choose width=40 height=30
        button "Last" testId="last" height=30
"#;

#[test]
fn radio_arrows_leave_the_inert_pages_state_and_handlers_untouched() {
    // Modal only: under a popover the page is live (`alert_popover_tests`).
    {
        let kind = Kind::Dialog;
        for checked in [false, true] {
            let mut p = boot(RADIOS, kind);
            if checked {
                tap(&mut p, "page");
            }
            tap(&mut p, "open");
            assert_eq!(p.focus, Some(id(&p, "a")));
            for (key, next) in [
                ("ArrowLeft", "b"),
                ("ArrowRight", "a"),
                ("ArrowDown", "b"),
                ("ArrowUp", "a"),
            ] {
                p.key_down(key, 0.);
                assert_eq!(p.focus, Some(id(&p, next)), "{key}");
                assert!(p.radio_checked(id(&p, next)));
                assert_ne!(p.radio_checked(id(&p, "a")), p.radio_checked(id(&p, "b")));
                assert!(!p.radio_checked(id(&p, "disabled")));
                assert_eq!(p.host.runner().slot("choice"), Some(&Value::str(next)));
                assert_eq!(p.radio_checked(id(&p, "page")), checked);
                for slot in ["pageInputs", "pageChanges"] {
                    assert_eq!(
                        p.host.runner().slot(slot),
                        Some(&Value::Number(if checked { 1. } else { 0. }))
                    );
                }
            }
        }
    }
}

#[test]
fn radio_tab_stop_ignores_checked_and_unchecked_radios_on_the_inert_page() {
    // Modal only: under a popover the page is live (`alert_popover_tests`).
    {
        let kind = Kind::Dialog;
        for checked in [false, true] {
            let mut p = boot(&RADIOS.replace("autofocus=true", ""), kind);
            if checked {
                tap(&mut p, "page");
            }
            tap(&mut p, "open");
            assert_eq!(p.focus, Some(id(&p, "a")));
            for next in ["last", "a", "last", "a"] {
                p.key_down("Tab", 0.);
                assert_eq!(p.focus, Some(id(&p, next)));
            }
            assert_eq!(p.radio_checked(id(&p, "page")), checked);
        }
    }
}

#[test]
fn ime_composes_in_authored_fields_but_refuses_the_page_and_shape_surfaces() {
    let source = r#"component App
  state picked = false
  action pick
    picked = true
  view
    column width="100%" height="100%"
      input testId="page" width=120 height=30
      button "Open" testId="open" commandfor="box" command="show-modal" height=30
      button "Menu" testId="menu-open" popovertarget="menu" height=30
      dialog id="box" testId="box" width=220
        text "Enter a value"
        input testId="editor" autofocus=true width=200 height=30
        button "Apply" testId="apply" press=pick commandfor="box" command="close"
        button "Cancel" testId="cancel" commandfor="box" command="close"
      column id="menu" popover="auto" role="menu"
        button "Item" press=pick
"#;
    // Modal kinds only: under a popover the page is live (`alert_popover_tests`).
    for kind in [Kind::Dialog, Kind::Alert] {
        let mut p = boot(source, kind);
        tap(&mut p, "open");
        let editor = id(&p, "editor");
        assert_eq!(p.focus, Some(editor));
        let target = p
            .ime_target()
            .expect("the open box's field accepts composition");
        assert!(p.ime_begin(target));
        p.ime_preedit("に", Some((3, 3)));
        assert_eq!(p.field_text(editor), "に");
        p.ime_commit("日本語");
        assert_eq!(p.field_text(editor), "日本語");
        for next in ["apply", "cancel", "editor"] {
            p.key_down("Tab", 0.);
            assert_eq!(p.focus, Some(id(&p, next)));
        }
        p.focus = Some(id(&p, "page"));
        assert!(p.ime_target().is_none(), "stale page focus is inert");
        p.alert_close(0.);
        tap(&mut p, "menu-open");
        assert!(p.surface().is_some());
        p.focus = Some(id(&p, "page"));
        assert!(
            p.ime_target().is_none(),
            "a shape surface keeps the page inert"
        );
    }
}

#[test]
fn scrollbars_drag_and_page_only_scrollers_inside_the_authored_box() {
    let source = r#"component App
  state presses = 0
  action press
    presses = presses + 1
  view
    column width="100%" height="100%"
      button "Open" testId="open" commandfor="box" command="show-modal" height=30
      column testId="page" width=480 height=200 overflow-y="auto"
        button "Page content" press=press height=1000 flex-shrink=0
      dialog id="box" testId="box" width=220 height=180 overflow-y="auto"
        button "Content" press=press height=1000 flex-shrink=0
"#;
    // Modal kinds only: under a popover the page is live (`alert_popover_tests`).
    for kind in [Kind::Dialog, Kind::Alert] {
        for nested in [false, true] {
            let source = if kind == Kind::Alert {
                let content = if nested {
                    format!(
                        "        text \"{}\" testId=\"message\"\n        input width=200 height=30",
                        "Long message. ".repeat(200)
                    )
                } else {
                    "        text \"Enter a value\"\n        input width=200 height=1000 flex-shrink=0".into()
                };
                source.replace("height=180", "height=280").replace(
                    "        button \"Content\" press=press height=1000 flex-shrink=0",
                    &format!(
                        "{content}\n        button \"Apply\" press=press commandfor=\"box\" command=\"close\"\n        button \"Cancel\" commandfor=\"box\" command=\"close\""
                    ),
                )
            } else if nested {
                source.replace(
                    "        button \"Content\" press=press height=1000 flex-shrink=0",
                    "        column testId=\"nested\" width=200 height=150 overflow-y=\"auto\"\n          button \"Content\" press=press height=1000 flex-shrink=0",
                )
            } else {
                source.into()
            };
            let mut p = boot(&source, kind);
            p.set_scrollbars(false);
            tap(&mut p, "open");
            let scroller = if nested && kind == Kind::Alert {
                p.host
                    .kernel()
                    .node(id(&p, "message"))
                    .unwrap()
                    .parent
                    .unwrap()
            } else {
                id(&p, if nested { "nested" } else { "box" })
            };
            let bar = *p
                .brush
                .scrollbars
                .bars
                .iter()
                .find(|b| b.id == scroller && b.vertical)
                .unwrap();
            assert!(bar.thumb.3 < bar.track.3);
            let (x, y) = (
                bar.thumb.0 + bar.thumb.2 / 2.,
                bar.thumb.1 + bar.thumb.3 / 2.,
            );
            p.pointer_down(x, y, 0.).unwrap();
            assert!(p.brush.scrollbars.drag.is_some());
            p.pointer_move(x, bar.track.1 + bar.track.3, 1.).unwrap();
            p.frame();
            p.pointer_up(x, bar.track.1 + bar.track.3, 2.).unwrap();
            assert_eq!(p.scroll_of(scroller).1, bar.max);
            p.frame();
            let bar = *p
                .brush
                .scrollbars
                .bars
                .iter()
                .find(|b| b.id == scroller && b.vertical)
                .unwrap();
            p.pointer_down(x, bar.track.1 + 1., 3.).unwrap();
            p.pointer_up(x, bar.track.1 + 1., 4.).unwrap();
            assert_eq!(p.scroll_of(scroller).1, (bar.max - bar.port.3).max(0.));
            p.frame();
            let page = id(&p, "page");
            let bar = *p
                .brush
                .scrollbars
                .bars
                .iter()
                .find(|b| b.id == page && b.vertical)
                .unwrap();
            let (x, y) = (
                bar.track.0 + bar.track.2 / 2.,
                bar.track.1 + bar.track.3 - 1.,
            );
            p.pointer_down(x, y, 5.).unwrap();
            p.pointer_up(x, y, 6.).unwrap();
            assert_eq!(p.scroll_of(page), (0., 0.));
            assert_eq!(p.host.runner().slot("presses"), Some(&Value::Number(0.)));
            assert_eq!(p.alert_root(), Some(id(&p, "box")));
        }
    }
}
