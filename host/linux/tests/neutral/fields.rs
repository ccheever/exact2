use super::{id, pixel, PainterChoice, Presenter};
use exact_linux::presenter::PointerKind;

pub(super) fn select_and_date_focus() {
    let mut failures = Vec::new();
    for scheme in ["light", "dark"] {
        for kind in ["select", "date", "time", "datetime-local"] {
            for bare in [false, true] {
                let tag = if kind == "select" {
                    "select".to_string()
                } else {
                    format!("input type=\"{kind}\"")
                };
                let child = if kind == "select" {
                    "        option \"One\" value=\"one\"\n"
                } else {
                    ""
                };
                let appearance = if bare { "none" } else { "auto" };
                let source = format!(
                    r##"component App
  state invalid = false
  state disabled = false
  action change(k: string)
    if k == "v"
      invalid = !invalid
  action focusField
    focus("field")
  action disable
    disabled = true
  view
    column width=300 height=280 padding=20 gap=10 align-items="flex-start" color-scheme="{scheme}" background-color="#204060"
      button "Before" testId="before"
      {tag} id="field" testId="field" width=160 height=40 appearance="{appearance}" accent-color="#008800" aria-invalid=(invalid ? "true" : "false") disabled=disabled key=change
{child}      button "Focus" press=focusField testId="focus"
      button "Disable" press=disable testId="disable"
      row width=20 height=20 background-color="-exact-system-red" testId="red"
"##
                );
                let plan = contract::compile(&source).unwrap();
                let (mut p, error) = Presenter::boot_with(
                    &plan.encode(),
                    (),
                    (300.0, 280.0),
                    1.0,
                    std::path::PathBuf::new(),
                    PainterChoice::Cpu,
                )
                .unwrap();
                let _roles = p.host().runner().bind_roles();
                assert!(error.is_none(), "{error:?}");
                let field = id(&p, "field");
                // The neutral border occupies pixel 0; a ring also paints pixel 1.
                let unfocused = pixel(&mut p, "field", 80.0, 1.0);
                let red = pixel(&mut p, "red", 10.0, 10.0);
                let context = format!("{scheme} {kind} appearance={appearance}");
                for programmatic in [false, true] {
                    focus(&mut p, programmatic);
                    for (step, invalid) in [false, true, false].into_iter().enumerate() {
                        assert_eq!(p.focus(), Some(field), "{context}");
                        let expected = if bare {
                            unfocused
                        } else if invalid {
                            red
                        } else {
                            [0, 136, 0]
                        };
                        let actual = pixel(&mut p, "field", 80.0, 1.0);
                        if actual != expected {
                            failures.push(format!("{context}, programmatic={programmatic}, invalid={invalid}: {actual:?} != {expected:?}"));
                        }
                        key(&mut p, "field", "Tab");
                        assert_ne!(p.focus(), Some(field), "{context}: blur");
                        assert_eq!(
                            pixel(&mut p, "field", 80.0, 1.0),
                            unfocused,
                            "{context}: blur, invalid={invalid}"
                        );
                        focus(&mut p, programmatic);
                        if step < 2 {
                            key(&mut p, "field", "v");
                        }
                    }
                }
                for pointer in [PointerKind::Touch, PointerKind::Mouse] {
                    p.set_pointer_kind(pointer);
                    let (x, y, w, h) = p.rect_of(field).unwrap();
                    p.press_at(x + w / 2.0, y + h / 2.0, 0.0);
                    assert_eq!(p.focus(), Some(field), "{context}: {pointer:?}");
                    let expected = if bare || pointer == PointerKind::Touch {
                        unfocused
                    } else {
                        [0, 136, 0]
                    };
                    let actual = pixel(&mut p, "field", 80.0, 1.0);
                    if actual != expected {
                        failures.push(format!(
                            "{context}, {pointer:?}: {actual:?} != {expected:?}"
                        ));
                    }
                    if kind == "select" {
                        p.press_at(0.0, 0.0, 0.0);
                    }
                }
                p.tap(id(&p, "disable")).unwrap();
                p.tap(id(&p, "focus")).unwrap();
                p.run_commands(<()>::default);
                assert_ne!(p.focus(), Some(field), "{context}: disabled focus refused");
                let disabled = pixel(&mut p, "field", 80.0, 1.0);
                key(&mut p, "before", "Tab");
                assert_ne!(p.focus(), Some(field), "{context}: disabled Tab skipped");
                assert_eq!(pixel(&mut p, "field", 80.0, 1.0), disabled);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn focus(p: &mut Presenter<()>, programmatic: bool) {
    if programmatic {
        p.tap(id(p, "focus")).unwrap();
        p.run_commands(<()>::default);
    } else {
        key(p, "before", "Tab");
    }
}

fn key(p: &mut Presenter<()>, target: &str, key: &str) {
    let target = id(p, target);
    p.type_key(target, key, key, true, false).unwrap();
    p.type_key(target, key, key, false, false).unwrap();
}
