//! @ref LLP 1121 D9 (stage 8 C2) — `tamber-unreachable`: for each target the
//! app ships, each control parameter it sets that the target's host does
//! not honour there (the schema's control-role table), once a target,
//! control and parameter, in one line for each override; `best-effort` says
//! nothing; a node's own row wins over the role.

use std::path::PathBuf;

fn app(name: &str, targets: &str, design: &str, view: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-controls-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.json"), format!("{{\"app\": {{\"id\": \"dev.test.controls\", \"name\": \"Controls\"}}, \"host\": {{{targets}}}}}")).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  // One colour in both schemes.\n{design}"),
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), view).unwrap();
    dir.join("app.contract")
}

/// The `tamber-unreachable` messages, sorted.
fn unreachable(path: &std::path::Path) -> Vec<String> {
    let (plan, warnings) = contract::compile_path_all_warned(path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    let mut out: Vec<String> = warnings
        .iter()
        .filter(|w| w.id == "tamber-unreachable")
        .map(|w| w.message.clone())
        .collect();
    out.sort();
    out
}

const SWITCHES: &str = "component App\n  state on = true\n  action check(checked: bool)\n    on = checked\n  view\n    column accent-color=\"accent.brand\"\n      input type=\"checkbox\" switch checked=on change=check aria-label=\"A\"\n      input type=\"checkbox\" switch checked=on change=check aria-label=\"B\"\n";

#[test]
fn an_accent_on_switches_is_unreachable_on_the_mac_only() {
    let path = app(
        "switch",
        "\"macos\": {}, \"web\": {}",
        "  accent.brand = #e0218a\n",
        SWITCHES,
    );
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("accent-color on macOS switches: NSSwitch draws the system accent"),
        "{said:?}"
    );
    // What ships the Mac, which `exact new` writes unasked, and how to stop.
    assert!(
        said[0].ends_with(
            "The app ships macos (app.json `host.macos`); not shipping it? delete those keys"
        ),
        "{said:?}"
    );
    // A control and its disabled state that the Mac leaves alike: one line.
    let both = SWITCHES.replace("aria-label=\"B\"", "aria-label=\"B\" disabled=true");
    let path = app(
        "switch-disabled",
        "\"macos\": {}",
        "  accent.brand = #e0218a\n",
        &both,
    );
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("accent-color on macOS switches and disabled switches: NSSwitch"),
        "{said:?}"
    );
    // Shipping the web alone: nothing.
    let path = app(
        "switch-web",
        "\"web\": {}",
        "  accent.brand = #e0218a\n",
        SWITCHES,
    );
    assert_eq!(unreachable(&path), Vec::<String>::new());
}

#[test]
fn best_effort_is_silent_and_an_unset_parameter_is_not_judged() {
    // The Mac's checkbox is exact and its disabled prominent button best-effort
    // (measured in a key window, stage 9 F2): no word for either.
    let view = "component App\n  state on = true\n  action check(checked: bool)\n    on = checked\n  action go\n    on = on\n  view\n    column\n      input type=\"checkbox\" checked=on change=check aria-label=\"A\" accent-color=\"accent.brand\"\n      button \"Go\" buttonStyle=\"bordered-prominent\" press=go disabled=true accent-color=\"accent.brand\"\n";
    let path = app(
        "checkbox",
        "\"macos\": {}",
        "  accent.brand = #e0218a\n",
        view,
    );
    assert_eq!(unreachable(&path), Vec::<String>::new());
    // A disabled checkbox under an accent is none on the Mac (AppKit draws it grey): said.
    let disabled = view.replace(
        "change=check aria-label",
        "change=check disabled=true aria-label",
    );
    let path = app(
        "checkbox-disabled",
        "\"macos\": {}",
        "  accent.brand = #e0218a\n",
        &disabled,
    );
    let said = unreachable(&path);
    assert!(
        said.iter().any(|s| s.contains("disabled checkbox")),
        "{said:?}"
    );
    // A switch with no accent anywhere: nothing set, nothing unreachable.
    let plain = SWITCHES.replace(" accent-color=\"accent.brand\"", "");
    let path = app(
        "plain",
        "\"macos\": {}",
        "  accent.brand = #e0218a\n",
        &plain,
    );
    assert_eq!(unreachable(&path), Vec::<String>::new());
}

#[test]
fn a_field_role_is_judged_unless_the_field_writes_its_own_row() {
    let design = "  Field = #fff1a8\n";
    let view = "component App\n  state words = \"a\"\n  action write(value: string)\n    words = value\n  view\n    input value=words change=write aria-label=\"Words\"\n";
    let path = app("field", "\"macos\": {}", design, view);
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(
            "Field on macOS fields: AppKit's bezeled NSTextField draws its own ground"
        ),
        "{said:?}"
    );
    let own = view.replace(
        "aria-label=\"Words\"",
        "aria-label=\"Words\" background-color=\"#ffffff\"",
    );
    let path = app("field-own", "\"macos\": {}", design, &own);
    assert_eq!(unreachable(&path), Vec::<String>::new());
}

#[test]
fn a_disabled_prominent_button_on_ios_says_the_platform_draws_it() {
    let view = "component App\n  state busy = true\n  action go\n    busy = !busy\n  view\n    button \"Post\" buttonStyle=\"prominent-glass\" press=go disabled=busy\n";
    let path = app("glass", "\"ios\": {}", "  AccentColor = #e0218a\n", view);
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("accent-color on iOS disabled prominent buttons: iOS draws a disabled prominent glass button grey"), "{said:?}");
}

/// Review: a class may set the button's style; a conditional class's
/// accent counts; a type in capitals is the same control; a control of the
/// author's own (`appearance="none"`) is not judged.
#[test]
fn classes_conditions_and_appearance_none() {
    let design = "  accent.brand = #e0218a\n";
    let view = "style Primary\n  buttonStyle=\"bordered-prominent\"\n\nstyle Brand\n  accent-color=\"accent.brand\"\n\nstyle Plain\n  opacity=1\n\ncomponent App\n  state on = true\n  state busy = true\n  action check(checked: bool)\n    on = checked\n  action go\n    busy = !busy\n  view\n    column class=(on ? Brand : Plain)\n      input type=\"CHECKBOX\" switch checked=on change=check aria-label=\"A\"\n      button \"Post\" class=Primary press=go disabled=busy\n      input type=\"checkbox\" switch appearance=\"none\" checked=on change=check aria-label=\"B\"\n";
    let path = app("classes", "\"macos\": {}, \"ios\": {}", design, view);
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("macOS switches (NSSwitch"), "{said:?}");
    assert!(
        said[0].contains("iOS disabled prominent buttons (iOS draws"),
        "{said:?}"
    );
    // The `appearance="none"` switch alone: nothing.
    let own = view
        .replace(
            "      input type=\"CHECKBOX\" switch checked=on change=check aria-label=\"A\"\n",
            "",
        )
        .replace(
            "      button \"Post\" class=Primary press=go disabled=busy\n",
            "",
        );
    let path = app("own", "\"macos\": {}", design, &own);
    assert_eq!(unreachable(&path), Vec::<String>::new());
}

/// One declaration that several hosts' controls leave alone is one line
/// (the 2026-10-30 ruling): it names the hosts, each part's controls and
/// why, the hosts that agree once, and how to stop shipping each.
#[test]
fn one_declaration_is_one_line_naming_its_hosts() {
    let view = "component App\n  state on = true\n  state size = \"m\"\n  action check(checked: bool)\n    on = checked\n  action pick(value: string)\n    size = value\n  action go\n    on = on\n  view\n    column accent-color=\"accent.brand\"\n      input type=\"checkbox\" switch checked=on change=check aria-label=\"A\"\n      select value=size change=pick aria-label=\"Size\"\n        option \"Medium\" value=\"m\"\n      button \"Post\" buttonStyle=\"prominent-glass\" press=go disabled=true\n";
    let path = app(
        "collapse",
        "\"web\": {}, \"ios\": {}, \"macos\": {}, \"linux\": {}, \"windows\": {}",
        "  accent.brand = #e0218a\n",
        view,
    );
    let said = unreachable(&path);
    assert_eq!(said.len(), 1, "{said:?}");
    let line = &said[0];
    assert!(
        line.starts_with(
            "accent-color on iOS, macOS, the web, Linux and Windows: the platform's look stands on "
        ),
        "{line}"
    );
    for part in [
        "macOS switches (NSSwitch draws the system accent)",
        "macOS selects (NSPopUpButton takes no tint)",
        "the web's selects (a browser's select takes no accent-color)",
        "Linux and Windows selects (the neutral look, as the web's, draws a closed select without the accent)",
        "iOS disabled prominent buttons (iOS draws a disabled prominent glass button grey",
        "the web's, Linux and Windows disabled prominent buttons (the neutral look draws",
    ] {
        assert!(line.contains(part), "{part} in {line}");
    }
    assert!(
        line.contains("(the first at line 12; LLP 1121 D9)"),
        "{line}"
    );
    assert!(
        line.ends_with("; not shipping one? `bun exact.mjs update --without linux` removes linux; otherwise delete its keys"),
        "{line}"
    );
}

/// Review round 2: `accent-color="auto"` is the platform's accent, no
/// override; the node's own `buttonStyle` and `appearance` hold over its
/// class's; a timed `progress start=…` is a bar; a field's text colour
/// every class it may take gives exempts it, one a branch may clear does not.
#[test]
fn auto_precedence_timed_progress_and_sure_rows() {
    let design = "  accent.brand = #e0218a\n  Field = #fff1a8\n";
    let auto = "component App\n  state on = true\n  action check(checked: bool)\n    on = checked\n  view\n    column accent-color=\"accent.brand\"\n      input type=\"checkbox\" switch accent-color=\"auto\" checked=on change=check aria-label=\"A\"\n";
    assert_eq!(
        unreachable(&app("auto", "\"macos\": {}", design, auto)),
        Vec::<String>::new()
    );
    let own = "style Own\n  appearance=\"none\"\n\nstyle Loud\n  buttonStyle=\"prominent-glass\"\n\ncomponent App\n  state on = true\n  state busy = true\n  action check(checked: bool)\n    on = checked\n  action go\n    busy = !busy\n  view\n    column accent-color=\"accent.brand\"\n      input type=\"checkbox\" switch class=Own appearance=\"auto\" checked=on change=check aria-label=\"A\"\n      button \"Post\" class=Loud buttonStyle=\"plain\" press=go disabled=busy\n";
    let said = unreachable(&app(
        "precedence",
        "\"macos\": {}, \"ios\": {}",
        design,
        own,
    ));
    assert!(
        said.iter()
            .any(|m| m.starts_with("accent-color on macOS switches")),
        "{said:?}"
    );
    assert!(
        !said.iter().any(|m| m.contains("prominent buttons")),
        "{said:?}"
    );
    let timed =
        "component App\n  view\n    progress start=0 end=1000 accent-color=\"accent.brand\"\n";
    // A timed progress: the web's bar (exact), the Mac's spinner (none;
    // iOS's is best-effort since the spinner rule, so says nothing).
    assert!(unreachable(&app("timed-web", "\"web\": {}", design, timed)).is_empty());
    assert!(unreachable(&app("timed", "\"macos\": {}", design, timed))
        .iter()
        .any(|m| m.contains("macOS spinners")));

    let both = "style Red\n  background-color=\"#ff0000\"\n\nstyle Blue\n  background-color=\"#0000ff\"\n\ncomponent App\n  state flag = true\n  state words = \"a\"\n  action write(value: string)\n    words = value\n  view\n    input class=(flag ? Red : Blue) value=words change=write aria-label=\"W\"\n";
    assert!(unreachable(&app("both", "\"macos\": {}", design, both)).is_empty());
    let cleared = "component App\n  state flag = true\n  state words = \"a\"\n  action write(value: string)\n    words = value\n  view\n    input background-color=(flag ? \"#ff0000\" : none) value=words change=write aria-label=\"W\"\n";
    assert_eq!(
        unreachable(&app("cleared", "\"macos\": {}", design, cleared)).len(),
        1
    );
}

/// Review round 3: each branch of a conditional class holds on its own; a
/// node's own row that may clear wins over its class's; a switch that is
/// native in one branch is judged.
#[test]
fn branches_hold_apart() {
    let design = "  accent.brand = #e0218a\n  Field = #fff1a8\n";
    let head = "style Primary\n  buttonStyle=\"filled\"\n\nstyle Plain\n  buttonStyle=\"plain\"\n\nstyle Red\n  background-color=\"#ff0000\"\n\ncomponent App\n  state flag = true\n  state on = true\n  state words = \"a\"\n  action check(checked: bool)\n    on = checked\n  action write(value: string)\n    words = value\n  action go\n    flag = !flag\n  view\n    column accent-color=\"accent.brand\"\n";
    let button = format!(
        "{head}      button \"Post\" class=(flag ? Primary : Plain) press=go disabled=flag\n"
    );
    assert!(
        unreachable(&app("branch-button", "\"ios\": {}", design, &button))
            .iter()
            .any(|m| m.contains("disabled prominent buttons"))
    );
    let field = format!("{head}      input class=Red background-color=(flag ? \"#ff0000\" : none) value=words change=write aria-label=\"W\"\n");
    assert!(
        unreachable(&app("branch-field", "\"macos\": {}", design, &field))
            .iter()
            .any(|m| m.starts_with("Field on"))
    );
    let switch = format!("{head}      input type=\"checkbox\" switch appearance=(flag ? \"auto\" : \"none\") checked=on change=check aria-label=\"S\"\n");
    assert!(
        unreachable(&app("branch-switch", "\"macos\": {}", design, &switch))
            .iter()
            .any(|m| m.contains("macOS switches"))
    );
}

/// Review round 3 (Grok): `role="switch"` is a switch (a conditional
/// `type` chooses only between text fields, which compile refuses otherwise); an accent that is `auto` in one branch
/// and authored in the other is set.
#[test]
fn role_switch_conditional_type_and_accent() {
    let design = "  accent.brand = #e0218a\n";
    let base = "component App\n  state flag = true\n  state on = true\n  state level = 5\n  action check(checked: bool)\n    on = checked\n  action slide(value: number)\n    level = value\n  view\n    column accent-color=\"accent.brand\"\n";
    let role = format!("{base}      input type=\"checkbox\" role=\"switch\" checked=on change=check aria-label=\"S\"\n");
    assert!(unreachable(&app("role", "\"macos\": {}", design, &role))
        .iter()
        .any(|m| m.contains("macOS switches")));
    let mixed = "component App\n  state flag = true\n  state on = true\n  action check(checked: bool)\n    on = checked\n  view\n    input type=\"checkbox\" switch accent-color=(flag ? \"auto\" : \"#e0218a\") checked=on change=check aria-label=\"S\"\n";
    assert!(unreachable(&app("mixed", "\"macos\": {}", design, mixed))
        .iter()
        .any(|m| m.contains("macOS switches")));
}

/// Final check: a branch's control and accent go together: a prominent
/// branch with the platform's accent and a plain branch with an authored
/// one is no disabled prominent button with an authored accent.
#[test]
fn a_branch_is_judged_whole() {
    let view = "style Primary\n  buttonStyle=\"filled\"\n  accent-color=\"auto\"\n\nstyle Plain\n  buttonStyle=\"plain\"\n  accent-color=\"#e0218a\"\n\ncomponent App\n  state flag = true\n  action go\n    flag = !flag\n  view\n    button \"Post\" class=(flag ? Primary : Plain) press=go disabled=flag\n";
    assert_eq!(
        unreachable(&app("whole", "\"ios\": {}", "  ink = #000000\n", view)),
        Vec::<String>::new()
    );
}

/// Stage 8 G: an app with twenty warnings of other kinds still hears of an
/// unreachable control parameter (it comes after the cap, as one_scheme).
#[test]
fn unreachable_survives_the_warning_cap() {
    let mut view = String::from("component App\n  state on = true\n  action check(checked: bool)\n    on = checked\n  view\n    column accent-color=\"accent.brand\"\n      input type=\"checkbox\" switch checked=on change=check aria-label=\"S\"\n");
    for i in 0..25 {
        view.push_str(&format!(
            "      column role=\"dialog\" testId=\"d{i}\"\n        text \"d\"\n"
        ));
    }
    let said = unreachable(&app(
        "cap",
        "\"macos\": {}",
        "  accent.brand = #e0218a\n",
        &view,
    ));
    assert!(
        said.iter().any(|m| m.contains("macOS switches")),
        "{said:?}"
    );
}

/// The `tamber-contrast` messages of a compile.
fn contrast(path: &std::path::Path) -> Vec<String> {
    let (plan, warnings) = contract::compile_path_all_warned(path, false);
    plan.unwrap_or_else(|e| panic!("{e:?}"));
    warnings
        .iter()
        .filter(|w| w.id == "tamber-contrast")
        .map(|w| w.message.clone())
        .collect()
}

// @ref LLP 1116.019 D3 — a filled button keeps UIKit's white label, so a pale
// accent under one is said when the app compiles, whatever screens a drive reaches.
#[test]
fn a_pale_accent_under_a_prominent_button_is_said_once() {
    let view = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      button \"Save\" buttonStyle=\"bordered-prominent\" press=go\n      button \"Again\" buttonStyle=\"filled\" press=go\n";
    // Round 28's mood journal: a deep teal in light, a pale one in dark.
    let pale = "  AccentColor = light-dark(#3c7467, #a8cac4)\n";
    let said = contrast(&app("accent-pale", "\"ios\": {}", pale, view));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with(
            "`AccentColor` is 1.76:1 under a prominent button's white label in dark: below 3:1"
        ),
        "{said:?}"
    );
    // A deep accent, or no prominent button: nothing.
    let deep = "  AccentColor = light-dark(#3c7467, #2f7d6d)\n";
    assert_eq!(
        contrast(&app("accent-deep", "\"ios\": {}", deep, view)),
        Vec::<String>::new()
    );
    let plain = view
        .replace(" buttonStyle=\"bordered-prominent\"", "")
        .replace(" buttonStyle=\"filled\"", "");
    assert_eq!(
        contrast(&app("accent-plain", "\"ios\": {}", pale, &plain)),
        Vec::<String>::new()
    );
}
