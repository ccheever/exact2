//! @ref LLP 1047 D2 — what a plan uses is read from its rows, never declared.

use exact_runner::{uses, Capability, Uses};

fn used(source: &str) -> Uses {
    uses(&contract::compile(source).unwrap_or_else(|e| panic!("{e}")))
}

#[test]
fn markdown_is_a_markup_prop_that_can_be_markdown() {
    let plain = used("component A\n  view\n    text \"**b**\"\n");
    assert_eq!(plain, Uses::NONE);
    let constant = used("component A\n  view\n    text \"**b**\" markup=\"markdown\"\n");
    assert!(constant.has(Capability::Markdown));
    assert_eq!(constant.to_string(), "markdown");
    // Another constant selects nothing; a computed one might be Markdown.
    let other = used("component A\n  view\n    text \"**b**\" markup=\"plain\"\n");
    assert_eq!(other, Uses::NONE);
    let computed = used(
        "component A\n  state rich = true\n  view\n    text \"**b**\" markup=(rich ? \"markdown\" : \"plain\")\n",
    );
    assert!(computed.has(Capability::Markdown));
}

#[test]
fn motion_is_a_spring_or_a_gesture_that_holds_a_value() {
    let css =
        used("component A\n  view\n    text \"a\" opacity=0.5 transition=\"opacity 200ms ease\"\n");
    assert_eq!(css, Uses::NONE, "CSS plays an easing transition");
    let spring = used(
        "component A\n  view\n    text \"a\" scale=1.5 transition=\"scale spring(180, 12, 1)\"\n",
    );
    assert!(spring.has(Capability::Motion));
    let swipe = used(
        "component A\n  state n = 0\n  action swipe writes n\n    n = n + 1\n  view\n    text \"a\" swiperight=swipe\n",
    );
    assert!(swipe.has(Capability::Motion));
    assert!(
        !swipe.has(Capability::Drag),
        "a swipe holds a value but tracks no handle"
    );
    let drag = used(
        "component A\n  view\n    column id=\"sheet\" height=100\n      column heightDragFor=\"sheet\"\n",
    );
    assert!(drag.has(Capability::Motion) && drag.has(Capability::Drag));
    assert_eq!(
        spring.with(Capability::Markdown).to_string(),
        "markdown, motion"
    );
}

#[test]
fn collections_are_lists_the_host_windows() {
    let list = |attrs: &str| {
        used(&format!(
            "component A\n  resource rows = rows() as shape list<string>\n  view\n    list {attrs}\n      each x in rows key=x\n        column\n          text x\n"
        ))
    };
    assert_eq!(list("height=100"), Uses::NONE, "a plain list is the core's");
    assert!(list("virtualized=true height=100").has(Capability::Collections));
    assert_eq!(list("virtualized=false height=100"), Uses::NONE);
    assert!(
        list("virtualized=true estimated-item-height=20 height=100").has(Capability::Collections)
    );
}

#[test]
fn surfaces_are_a_canvas_with_one() {
    assert_eq!(used("component A\n  view\n    canvas\n"), Uses::NONE);
    assert!(used("component A\n  view\n    canvas surface=sky()\n").has(Capability::Surfaces));
    let record = used(
        "shape Hud\n  beacons: number\ncomponent App\n  resource hud = exactSurface(\"world\") as shape Hud\n  view\n    text `${hud.beacons}`\n",
    );
    assert!(
        record.has(Capability::Surfaces),
        "a surface's record is read without a canvas"
    );
}

#[test]
fn the_router_is_a_plan_with_routes() {
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Router));
    let routed = used("routes nav\n  tab home \"/\"\ncomponent App\n  view\n    text \"a\"\n");
    assert!(routed.has(Capability::Router));
}

#[test]
fn format_is_a_call_of_format_date_or_format_number_anywhere() {
    // `formatTime` is the core's, as `formatClockTime` was (LLP 1054.000.003 D1).
    let time = used("component A\n  view\n    text formatTime(0, 0, \"short\")\n");
    assert_eq!(time, Uses::NONE);
    let date = used("component A\n  view\n    text formatDate(0, 0, \"medium\")\n");
    assert_eq!(date.to_string(), "format");
    // In an action's body, a derive, a callback and an inlined `fn` too.
    let action = used(
        "component A\n  state s = \"\"\n  action set writes s\n    s = formatNumber(1250, \"compact\")\n  view\n    button s press=set\n",
    );
    assert!(action.has(Capability::Format));
    let callback = used(
        "component A\n  resource xs = xs() as shape list<number>\n  derive ys = map(xs, x => formatNumber(x, \"compact\"))\n  view\n    text join(ys, \",\")\n",
    );
    assert!(callback.has(Capability::Format));
    let wrapped = used(
        "fn count(n: number): string = formatNumber(n, \"compact\")\ncomponent A\n  view\n    text count(3)\n",
    );
    assert!(wrapped.has(Capability::Format));
}

#[test]
fn materials_and_backdrop_filter_are_linked_by_use() {
    // @ref LLP 1053.000 §2 — the web core carries neither unless a plan names one.
    let plain = used("component A\n  view\n    text \"a\" white-space=\"pre\"\n");
    assert!(!plain.has(Capability::Materials) && !plain.has(Capability::Backdrop));
    assert!(
        used("component A\n  view\n    box backgroundMaterial=\"thin\"\n")
            .has(Capability::Materials)
    );
    assert!(
        used("component A\n  view\n    box backdrop-filter=\"blur(4px)\"\n")
            .has(Capability::Backdrop)
    );
}

#[test]
fn drag_timelines_are_linked_by_use() {
    // @ref LLP 1057.003 — the web core carries their grammar only when a plan sets a row.
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Timelines));
    for row in [
        "drag-timeline=\"--dismiss\"",
        "animation-timeline=\"--dismiss\"",
        "animation-range=\"0px 300px\"",
    ] {
        let set = used(&format!("component A\n  view\n    box {row}\n"));
        assert!(
            set.has(Capability::Timelines) && set.has(Capability::Motion),
            "{row}"
        );
    }
}

#[test]
fn text_transform_is_linked_by_use() {
    // @ref LLP 1064 D5 — the web core carries no case tables unless a plan binds the row.
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::TextTransform));
    assert!(
        used("component A\n  view\n    text \"a\" text-transform=\"uppercase\"\n")
            .has(Capability::TextTransform)
    );
    assert!(used(
        "style S\n  text-transform=\"capitalize\"\ncomponent A\n  view\n    text \"a\" class=S\n"
    )
    .has(Capability::TextTransform));
}

#[test]
fn filter_and_clip_path_are_linked_by_use() {
    // @ref LLP 1055.000 D10, D14 — the web core carries neither grammar unless a plan binds one.
    assert!(!used("component A\n  view\n    box opacity=0.5\n").has(Capability::Effects));
    assert!(used("component A\n  view\n    box filter=\"blur(2px)\"\n").has(Capability::Effects));
    assert!(used("component A\n  view\n    box clip-path=\"none\"\n").has(Capability::Effects));
}

#[test]
fn a_set_names_what_it_holds_beyond_another() {
    let markdown = Uses::NONE.with(Capability::Markdown);
    assert!(markdown.beyond(markdown).is_empty());
    assert_eq!(markdown.beyond(Uses::NONE), markdown);
    assert!(Uses::NONE.beyond(markdown).is_empty());
    assert_eq!(Uses::NONE.to_string(), "");
}

#[test]
fn share_is_a_plan_that_runs_the_command() {
    // @ref LLP 1069.003 — linked by use on the web (the web core's size).
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Share));
    let shares = used(
        "component A\n  action send\n    share(text=\"hello\")\n  view\n    button \"Share\" press=send\n",
    );
    assert!(shares.has(Capability::Share));
}

#[test]
fn documents_are_a_plan_that_runs_save_file_or_a_file_picker() {
    // @ref LLP 1069.010 — linked by use on the web (the web core's size).
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Documents));
    let picks = used(
        "component A\n  action open\n    showOpenFilePicker(\"opened\")\n  view\n    button \"Open\" press=open\n",
    );
    assert!(picks.has(Capability::Documents) && !picks.has(Capability::Share));
}

#[test]
fn the_picker_is_a_file_input_or_show_picker() {
    // @ref LLP 1069.002 — linked by use on the web (the web core's size).
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Picker));
    let input = used("component A\n  view\n    input type=\"file\" accept=\"image/*\" id=\"f\"\n");
    assert!(input.has(Capability::Picker));
}
