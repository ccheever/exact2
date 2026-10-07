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
    let other = used("component A\n  view\n    text \"**b**\" markup=\"none\"\n");
    assert_eq!(other, Uses::NONE);
    let computed = used(
        "component A\n  state rich = true\n  view\n    text \"**b**\" markup=(rich ? \"markdown\" : \"none\")\n",
    );
    assert!(computed.has(Capability::Markdown));
}

#[test]
fn motion_is_a_spring_or_a_gesture_that_holds_a_value() {
    let css =
        used("component A\n  view\n    text \"a\" opacity=0.5 transition=\"opacity 200ms ease\"\n");
    assert_eq!(css, Uses::NONE, "CSS plays an easing transition");
    let spring = used(
        "component A\n  view\n    text \"a\" scale=1.5 transition=\"scale -exact-spring(180, 12, 1)\"\n",
    );
    assert!(spring.has(Capability::Motion));
    let swipe = used(
        "component A\n  state n = 0\n  action swipe\n    n = n + 1\n  view\n    text \"a\" swiperight=swipe\n",
    );
    assert!(swipe.has(Capability::Motion));
    assert!(
        !swipe.has(Capability::Drag),
        "a swipe holds a value but tracks no handle"
    );
    let pan = used(
        "component A\n  state n = 0\n  action move(dx: number, dy: number)\n    n = n + dx\n  view\n    text \"a\" pan=move\n",
    );
    assert_eq!(pan, Uses::NONE, "a pan commits state; it holds nothing");
    let release = used(
        "component A\n  state n = 0\n  action move(dx: number, dy: number)\n    n = n + dx\n  view\n    text \"a\" pan=move panrelease=move\n",
    );
    assert!(
        release.has(Capability::Motion) && !release.has(Capability::Drag),
        "a pan's release velocity is the engine's tracker (LLP 1057 §10.6)"
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
        "component A\n  state s = \"\"\n  action set\n    s = formatNumber(1250, \"compact\")\n  view\n    button s press=set\n",
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

/// LLP 1088 D2: `toLowerCase` reaches the case tables `text-transform`
/// links, so a call links them; `slice` and `replaceAll` need nothing.
#[test]
fn to_lower_case_links_the_case_tables_and_slice_links_nothing() {
    let cut = used(
        "component A\n  state s = \"Ab\"\n  view\n    text replaceAll(slice(s, 1), \"b\", \"c\")\n",
    );
    assert_eq!(cut, Uses::NONE);
    let lower = used("component A\n  state s = \"Ab\"\n  view\n    text toLowerCase(s)\n");
    assert_eq!(lower.to_string(), "text_transform");
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
        "-exact-drag-timeline=\"--dismiss\"",
        "animation-timeline=\"--dismiss\"",
        "animation-range=\"0px 300px\"",
        "timeline-scope=\"--dismiss\"",
        "timeline-scope=\"all\"",
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
fn css_animations_are_linked_by_use() {
    // @ref LLP 1055 D5 — the web core carries neither grammar unless a plan animates.
    assert!(!used("component A\n  view\n    box opacity=0.5\n").has(Capability::Animations));
    // `none` parses too, so a binding alone links the grammar.
    assert!(used("component A\n  view\n    box animation=\"none\"\n").has(Capability::Animations));
    assert!(used(
        "keyframes fade\n  from opacity=0\ncomponent A\n  view\n    box animation=\"fade 1s\"\n"
    )
    .has(Capability::Animations));
    assert!(
        used("component A\n  view\n    box -exact-exit-animation=\"none\"\n")
            .has(Capability::Animations)
    );
}

#[test]
fn gradients_are_linked_by_use() {
    // @ref LLP 1066 — the web core carries no gradient grammar unless a plan binds the row.
    assert!(!used("component A\n  view\n    box opacity=0.5\n").has(Capability::Gradients));
    assert!(used(
        "component A\n  view\n    view background-image=\"linear-gradient(#000, #fff)\"\n"
    )
    .has(Capability::Gradients));
}

#[test]
fn data_words_and_tabs_are_linked_by_use() {
    // @ref LLP 1075.003 §3.3, §3.7 — the web core carries neither the
    // `dataset` reading nor the tabs' walk unless a plan binds the row.
    let plain = used("component A\n  view\n    box testId=\"a\"\n");
    assert!(!plain.has(Capability::Dataset) && !plain.has(Capability::Tabs));
    let words = used("component A\n  view\n    box data-screen=\"home\"\n");
    assert_eq!(words.to_string(), "dataset");
    let tabs = used(
        "component A\n  view\n    column\n      row role=\"tablist\"\n        button role=\"tab\" aria-controls=\"one\"\n          text \"One\"\n      column role=\"tabpanel\" id=\"one\"\n",
    );
    assert_eq!(tabs.to_string(), "tabs");
}

#[test]
fn grid_grammars_are_linked_by_any_grid_row() {
    assert!(!used("component A\n  view\n    box display=\"grid\"\n").has(Capability::Grid));
    for (row, value, alternative) in [
        ("grid-template-columns", "1fr 2fr", "auto"),
        ("grid-template-rows", "auto 40px", "25%"),
        ("grid-column", "1 / span 2", "auto"),
        ("grid-row", "rail / 2", "span 2"),
        ("grid-auto-flow", "column dense", "row"),
        ("justify-items", "safe center", "stretch"),
    ] {
        let literal = used(&format!("component A\n  view\n    box {row}=\"{value}\"\n"));
        assert!(literal.has(Capability::Grid), "literal {row}");
        let dynamic = used(&format!(
            "component A\n  state on = true\n  view\n    box {row}=(on ? \"{value}\" : \"{alternative}\")\n"
        ));
        assert!(dynamic.has(Capability::Grid), "dynamic {row}");
    }
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
fn notifications_are_a_plan_that_runs_show_or_close_notification() {
    // Linked by use on the web (QUEUE.md's standing rule: the web cores' size).
    assert!(!used("component A\n  view\n    text \"a\"\n").has(Capability::Notifications));
    for call in [
        "showNotification(title=\"Stretch\")",
        "closeNotification(\"stretch\")",
    ] {
        let notifies = used(&format!(
            "component A\n  action go\n    {call}\n  view\n    button \"Go\" press=go\n"
        ));
        assert!(notifies.has(Capability::Notifications) && !notifies.has(Capability::Share));
    }
    assert_eq!(
        Uses::NONE.with(Capability::Notifications).to_string(),
        "notifications"
    );
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

#[test]
fn svg_islands_are_a_mask_a_filter_element_or_a_filter_on_an_svg_element() {
    // @ref LLP 1055.000 D10, D14 — what makes a native host open its island module.
    let islands = |s: &str| {
        exact_runner::svg_islands(&contract::compile(s).unwrap_or_else(|e| panic!("{e}")))
    };
    assert!(!islands("component A\n  view\n    svg viewBox=\"0 0 10 10\"\n      rect width=10 height=10 fill=\"#f00\"\n"));
    assert!(
        !islands("component A\n  view\n    box filter=\"blur(2px)\"\n"),
        "a box's filter is not an island"
    );
    assert!(islands("component A\n  view\n    svg viewBox=\"0 0 10 10\"\n      rect width=10 height=10 filter=\"blur(1px)\"\n"));
    let mask = "component A\n  view\n    svg viewBox=\"0 0 10 10\"\n      defs\n        mask id=\"m\"\n          rect width=5 height=5 fill=\"#fff\"\n      rect width=10 height=10 mask=\"url(#m)\"\n";
    assert!(islands(mask));
    // A mask alone is an island and no filter (no GPU filter pipelines).
    let filters = |s: &str| {
        exact_runner::svg_filters(&contract::compile(s).unwrap_or_else(|e| panic!("{e}")))
    };
    assert!(!filters(mask));
    assert!(filters("component A\n  view\n    svg viewBox=\"0 0 10 10\"\n      g filter=\"blur(1px)\"\n        rect width=10 height=10\n"));
    assert!(!filters(
        "component A\n  view\n    box filter=\"blur(2px)\"\n"
    ));
}

/// LLP 1047.001 D2: a grouped list (`list appearance="auto"`, which Contract
/// marks with `listStyle`) is a native capability; a plain list is not.
#[test]
fn a_grouped_list_is_a_list_whose_appearance_is_auto() {
    let plain = used("component A\n  view\n    list\n      text \"a\"\n");
    assert!(!plain.has(Capability::GroupedLists));
    let grouped = used(
        "component A\n  view\n    list appearance=\"auto\"\n      section\n        text \"a\"\n",
    );
    assert!(grouped.has(Capability::GroupedLists));
    assert_eq!(grouped.to_string(), "grouped_lists");
}

#[test]
fn svg_image_module_is_needed_for_documents_and_computed_sources_but_not_symbols() {
    let images =
        |s: &str| exact_runner::svg_images(&contract::compile(s).unwrap_or_else(|e| panic!("{e}")));
    assert!(!images(
        "component A\n  view\n    image \"symbol:sf/star\"\n"
    ));
    assert!(!images("component A\n  view\n    text \"hello\"\n"));
    assert!(images("component A\n  view\n    image \"icon.svg\"\n"));
    assert!(images(
        "component A\n  state source = \"icon.svg\"\n  view\n    image source\n"
    ));
}
