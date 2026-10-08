//! LLP 1081: every name an author writes in a CSS position is one of three
//! kinds — CSS's (bare), a browser's (its own prefix), or Exact's
//! (`-exact-`) — and the old spellings are refused with their new ones.

use contract_lower::style_names::{RENAMED_TOKENS, STYLE_NAMES};
use exact_kernel::corner::{CornerShape, KEYWORDS};
use exact_kernel::style::env::ENV_NAMES;
use exact_kernel::style::roles::{is_css_system, role};
use exact_kernel::Kernel;
use exact_kernel::COLOR_ROLES;
use exact_motion::parse::EASING_FUNCTIONS;
use exact_motion::{Keyframes, Property, Transitions};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn style_of(r: &Runner<NoData>, id: &str) -> exact_kernel::StyleProps {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .style
        .clone()
}

fn refusal(src: &str) -> String {
    contract::compile(src).unwrap_err().to_string()
}

/// D8: a provenance string's form, and the spelling its kind requires.
fn check(name: &str, provenance: &str) {
    let (kind, rest) = provenance.split_once(' ').unwrap_or((provenance, ""));
    match kind {
        "css" | "shipped" => {
            assert!(
                !rest.is_empty(),
                "{name}: `{provenance}` names no document or engine"
            );
            assert!(!name.starts_with('-'), "{name}: a CSS name is bare");
        }
        "browser" => {
            let mut words = rest.split(' ');
            let (engine, revision) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
            assert!(
                !revision.is_empty(),
                "{name}: a browser name is checked at a revision"
            );
            let prefixes: &[&str] = match engine {
                "webkit" => &["-webkit-", "-apple-"],
                other => panic!("{name}: unknown engine `{other}`"),
            };
            assert!(
                prefixes.iter().any(|p| name.starts_with(p)),
                "{name}: a {engine} name takes its prefix"
            );
        }
        "exact" => {
            assert!(
                rest.starts_with("LLP "),
                "{name}: an Exact name cites its LLP"
            );
            assert!(
                name.starts_with("-exact-"),
                "{name}: a name Exact invents is spelled `-exact-`"
            );
        }
        "internal" => {}
        other => panic!("{name}: unknown kind `{other}` in `{provenance}`"),
    }
}

#[test]
fn every_name_is_its_kind() {
    for (name, provenance, _) in STYLE_NAMES {
        check(name, provenance);
    }
    // The colour roles (LLP 1095 D2): CSS's system colours by their names,
    // Exact's as `-exact-<role>`, WebKit's aliases with WebKit's prefix.
    for r in COLOR_ROLES {
        if is_css_system(r) {
            check(r.name, "css CSS Color 4");
            assert_eq!(
                role(r.name)
                    .and_then(exact_kernel::style::roles::role_of)
                    .map(|x| x.name),
                Some(r.name)
            );
        } else {
            let spelled = format!("-exact-{}", r.name);
            check(&spelled, "exact LLP 1095");
            assert!(role(&spelled).is_some(), "{spelled}");
            assert_eq!(role(r.name), None, "the bare `{}` is not a colour", r.name);
        }
        if !r.alias.is_empty() {
            check(r.alias, "browser webkit bb06bdc9");
        }
    }
    for (name, _, provenance) in KEYWORDS {
        check(name, provenance);
    }
    for (name, provenance) in EASING_FUNCTIONS {
        check(name, provenance);
    }
    for (name, provenance) in ENV_NAMES {
        check(name, provenance);
    }
    let (clock, provenance) = exact_kernel::timeline::CLOCK_FUNCTION;
    check(clock, provenance);
}

#[test]
fn every_author_motion_name_is_a_style_name() {
    // Motion's author names (`Property::from_author_name`) are the style
    // table's spellings, so they share its kinds (LLP 1081 D8).
    for p in Property::ALL {
        if Property::from_author_name(p.name()) == Some(p) {
            assert!(
                STYLE_NAMES.iter().any(|(n, ..)| *n == p.name()),
                "motion's `{}` is not a style name",
                p.name()
            );
        }
    }
}

#[test]
fn no_old_spelling_is_accepted_by_any_table() {
    // The first fourteen are the property names; the rest are values.
    for (old, new) in RENAMED_TOKENS.iter().take(14) {
        assert!(
            contract_lower::tags::attr(old).is_none(),
            "`{old}` is still an attribute"
        );
        assert_eq!(
            contract_lower::tags::renamed(old),
            Some(*new),
            "`{old}`'s hint"
        );
    }
    assert!(CornerShape::check("-apple-continuous").is_err());
    assert!(CornerShape::check("-exact-continuous")
        .unwrap()
        .is_apple_continuous());
    assert_eq!(
        role("-apple-system-fill"),
        None,
        "WebKit has no bare systemFill"
    );
    assert!(role("-exact-fill").is_some());
    assert!(Transitions::parse("translate spring(300, 30, 1)").is_err());
    assert!(Transitions::parse("translate -exact-spring(300, 30, 1)").is_ok());
    assert!(Transitions::parse("tint-color 1s").is_err());
    assert!(Transitions::parse("-exact-tint-color 1s").is_ok());
    // D5: the host's custom property is not an author name in a transition,
    // and is still decoded where the plan stores a serialized keyframes rule.
    assert!(Transitions::parse("--exact-tint 1s").is_err());
    assert_eq!(Property::from_author_name("--exact-tint"), None);
    assert!(Keyframes::parse("to{--exact-tint:#ff0000}").is_ok());
}

#[test]
fn an_old_name_is_refused_with_its_new_one_in_every_context() {
    let app = |body: &str| format!("component App\n  view\n    column\n{body}");
    for (src, says) in [
        // an attribute
        (app("      view press-scale=0.96\n"), "-exact-press-scale"),
        (
            app("      image \"symbol:sf/star\" tint-color=\"#f00\"\n"),
            "-exact-tint-color",
        ),
        // a `style` class
        (
            format!(
                "style S\n  hover-effect=\"lift\"\n{}",
                app("      view class=S\n")
            ),
            "-exact-hover-effect",
        ),
        // a keyframe
        (
            format!(
                "keyframes k\n  to tint-color=\"#f00\"\n{}",
                app("      view animation=\"k 1s\"\n")
            ),
            "-exact-tint-color",
        ),
        // a transition list, mixed case, among others
        (
            app("      view transition=\"opacity 1s, Tint-Color 1s\"\n"),
            "-exact-tint-color",
        ),
        (
            app("      view transition=\"translate Spring(300, 30, 1)\"\n"),
            "-exact-spring(",
        ),
        (
            app("      view -exact-layout-transition=\"300ms spring(300, 30, 1)\"\n"),
            "-exact-spring(",
        ),
        // a value: a keyword, and a colour inside `light-dark()`
        (
            app("      view corner-shape=\"-apple-continuous\" border-radius=8\n"),
            "-exact-continuous",
        ),
        (
            app("      view background-color=\"light-dark(#fff, -apple-system-fill)\"\n"),
            "-exact-fill",
        ),
        (
            app("      view background-color=\"-APPLE-SYSTEM-FILL\"\n"),
            "-exact-fill",
        ),
        (
            app("      view background-color=\"platform-color(ios systemPinkColor, #ff2d55)\"\n"),
            "-exact-platform-color(",
        ),
        (
            app("      text \"a\" color=\"secondary-label\"\n"),
            "-exact-secondary-label",
        ),
        (
            app("      view background-color=\"light-dark(#fff, system-orange)\"\n"),
            "-exact-system-orange",
        ),
        // the host's own name
        (
            app("      view transition=\"--exact-tint 1s\"\n"),
            "-exact-",
        ),
    ] {
        let e = refusal(&src);
        assert!(e.contains(says), "{src}\n→ {e}");
    }
}

#[test]
fn a_class_sets_and_clears_a_renamed_row() {
    let r = boot(
        "style Pressed\n  -exact-press-scale=0.9\nstyle Plain\n  opacity=1\n\ncomponent App\n  state on = true\n  action flip\n    on = not on\n  view\n    column\n      view class=(on ? Pressed : Plain) press=flip testId=\"a\"\n      view class=Pressed -exact-press-scale=0.8 testId=\"b\"\n",
    );
    assert_eq!(style_of(&r, "a").press_scale, 0.9);
    assert_eq!(
        style_of(&r, "b").press_scale,
        0.8,
        "the element's own row overrides its class"
    );
    let k = r.kernel();
    let a = k.node_by_key(k.find_by_test_id("a")[0]).unwrap().id;
    let mut r = r;
    r.dispatch(a, exact_runner::Event::Press).unwrap();
    assert_eq!(
        style_of(&r, "a").press_scale,
        1.0,
        "the branch without the row clears it"
    );
}

#[test]
fn the_lexer_reads_an_exact_name_before_its_equals_and_a_negation_elsewhere() {
    let r = boot(
        "keyframes k\n  to -exact-tint-color=\"#f00\"\nstyle S\n  -exact-press-scale=0.9\n\ncomponent App\n  state w = 3\n  view\n    column\n      view -exact-press-scale=0.5 testId=\"a\"\n      view -exact-press-scale = 0.7 testId=\"b\"\n      view class=S testId=\"c\"\n      text `${10 -w}` testId=\"d\"\n      text `${w == 3}` testId=\"e\"\n      image \"symbol:sf/star\" animation=\"k 1s\" testId=\"f\"\n",
    );
    assert_eq!(style_of(&r, "a").press_scale, 0.5);
    assert_eq!(style_of(&r, "b").press_scale, 0.7);
    assert_eq!(style_of(&r, "c").press_scale, 0.9);
}

#[test]
fn review_cases_values_in_keyframes_composites_and_author_names() {
    let app = |body: &str| format!("component App\n  view\n    column\n{body}");
    // A keyframe value, and a bare role inside a gradient, get their hint.
    for (src, says) in [
        (
            format!(
                "keyframes k\n  to background-color=\"-apple-system-fill\"\n{}",
                app("      view animation=\"k 1s\"\n")
            ),
            "-exact-fill",
        ),
        (
            format!(
                "keyframes k\n  to color=\"secondary-label\"\n{}",
                app("      text \"a\" animation=\"k 1s\"\n")
            ),
            "-exact-secondary-label",
        ),
        (
            app("      view background-image=\"linear-gradient(secondary-label, red)\"\n"),
            "-exact-secondary-label",
        ),
        (
            app("      view animation-timeline=\"clock(Pending)\"\n"),
            "-exact-clock(",
        ),
        (
            app("      view animation-trigger=\"none\"\n"),
            "-exact-animation-trigger",
        ),
    ] {
        let e = refusal(&src);
        assert!(e.contains(says), "{src}\n→ {e}");
    }
    for (src, says) in [
        (
            format!(
                "keyframes k\n  to box-shadow=\"0px 1px 2px secondary-label\"\n{}",
                app("      view animation=\"k 1s\"\n")
            ),
            "-exact-secondary-label",
        ),
        (
            app("      view background-color=\"var(--EXACT-label, #000)\"\n"),
            "--exact-",
        ),
    ] {
        let e = refusal(&src);
        assert!(e.contains(says), "{src}\n→ {e}");
    }
    // A reference names the author's own element, whatever it is called.
    assert!(contract_lower::style_names::renamed_token(
        "url(#-apple-system-fill)",
        &[exact_kernel::StyleId::ClipPath]
    )
    .is_none());
    // An author's own name that looks like an old token stays theirs (D5).
    boot(&app("      view display=\"grid\" grid-template-columns=\"[underline-line-through] 1fr\"\n      view -exact-animation-trigger=\"none\"\n"));
}
