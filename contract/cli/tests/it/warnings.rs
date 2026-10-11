//! Lowering's warnings (LLP 1116 D6): what compiles, runs, and is not what
//! the author meant. They fail nothing; each names its repair; and what the
//! compiler cannot see through is given the benefit of the doubt.

/// The ids and lines of the warnings `src` earns, after checking that it
/// still compiles: a warning never fails a build.
fn warned(src: &str) -> Vec<(String, u32)> {
    let (result, warnings) = contract::compile_warned(src);
    if let Err(e) = result {
        panic!("a warning never fails a build: {e}\n{src}");
    }
    for w in &warnings {
        assert!(w.is_warning(), "{w}");
        assert!(w.to_string().contains(" warning ["), "{w}");
    }
    warnings
        .iter()
        .map(|w| (w.id.clone(), w.span.line))
        .collect()
}

/// A stack with one route whose first child is `header`, its lines at 9 on.
fn route(header: &str) -> String {
    let header: String = header.lines().map(|l| format!("          {l}\n")).collect();
    format!(
        "routes nav\n  home \"/\"\ncomponent App\n  state q = \"\"\n  action go\n    q = \"x\"\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\" width=\"100%\" height=\"100%\"\n      each e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` position=\"absolute\" inset=0\n          header display=\"flex\"\n{header}          scroll flex=1\n            text \"body\"\n"
    )
}

const UNPLACED: &str = "lower-header-unplaced";

#[test]
fn a_header_node_the_ios_bar_drops_is_named_with_its_repair() {
    // household-list's count beside its subtitle, recipe-finder's icon:
    // shown on the web, hidden with the header under UIKit's bar.
    let src = route(
        "  text \"List\" role=\"heading\" aria-level=1\n  text \"3 left\"\n  text \"synced\"\n  image \"symbol:heart\"\n  button press=go testId=\"add\"\n    text \"Add\"",
    );
    assert_eq!(
        warned(&src),
        [(UNPLACED.to_string(), 14), (UNPLACED.to_string(), 15)]
    );
    let (_, warnings) = contract::compile_warned(&src);
    assert!(
        warnings[0]
            .message
            .contains("this text, after the subtitle,")
            && warnings[1].message.contains("this `image`")
            && warnings[0]
                .message
                .contains("Move it below the `header`, into the route's content"),
        "{warnings:?}"
    );
    // Before the heading, a text has no place either; a second search
    // field or tablist neither.
    assert_eq!(
        warned(&route(
            "  text \"Hi\"\n  text \"List\" role=\"heading\" aria-level=1\n  input type=\"search\" value=q\n  input type=\"search\" value=q"
        )),
        [(UNPLACED.to_string(), 12), (UNPLACED.to_string(), 15)]
    );
    // A box of things the bar does not place is named once, as a box.
    assert_eq!(
        warned(&route(
            "  text \"List\" role=\"heading\" aria-level=1\n  text \"sub\"\n  row\n    text \"a\"\n    text \"b\""
        )),
        [(UNPLACED.to_string(), 14)]
    );
}

#[test]
fn what_the_bar_places_or_the_compiler_cannot_see_is_not_warned() {
    for header in [
        // The heading, its subtitle, a search field, a tablist, a button
        // that presses and one that opens a popover, a spacer between.
        "  text \"List\" role=\"heading\" aria-level=1\n  text \"3 left\"\n  view flex=1\n  input type=\"search\" value=q\n  row role=\"tablist\"\n    button role=\"tab\" aria-selected=true press=go\n      text \"A\"\n  button press=go testId=\"add\"\n    text \"Add\"\n  button popovertarget=\"menu\"\n    text \"More\"",
        // A heading's group: an avatar before it, a line of glyphs after.
        "  row\n    box background-color=\"-exact-system-blue\"\n      text \"AL\"\n    text \"Ada\" role=\"heading\" aria-level=1\n    row\n      image \"symbol:sf/bell.slash\"\n      text \"Muted\"",
        // A pressable group around the heading is the title, tapped.
        "  button press=go\n    text \"Ada\" role=\"heading\" aria-level=1\n    text \"online\"",
        // `when`, `each` and a computed role are given the benefit.
        "  text \"List\" role=\"heading\" aria-level=1\n  text \"sub\"\n  when q != \"\"\n    text q\n  each x in [\"a\"] key=x\n    text x\n  text \"c\" role=(q == \"\" ? \"note\" : \"status\")",
        // Shown on no host, or over the page when opened.
        "  text \"List\" role=\"heading\" aria-level=1\n  text \"sub\"\n  text \"hidden\" display=\"none\"\n  button popovertarget=\"m\"\n    text \"More\"\n  column id=\"m\" popover=\"auto\" role=\"menu\"\n    text \"Sort\"\n    button press=go popovertarget=\"m\" popovertargetaction=\"hide\" role=\"menuitem\"\n      text \"Go\"",
        // No heading, or two: the bar takes nothing and the header is shown.
        "  text \"one\"\n  image \"symbol:heart\"",
        "  text \"A\" role=\"heading\" aria-level=1\n  text \"B\" role=\"heading\" aria-level=2\n  image \"symbol:heart\"",
    ] {
        assert_eq!(warned(&route(header)), [], "{header}");
    }
    // A header that is not the route's first child is not its bar.
    let src = route("  text \"List\" role=\"heading\" aria-level=1\n  text \"a\"\n  text \"b\"")
        .replace(
            "          header display=\"flex\"\n",
            "          text \"first\"\n          header display=\"flex\"\n",
        );
    assert_eq!(warned(&src), []);
}

/// A root tab bar: tabs that name their panels, as polls and chat-rooms
/// wrote with one tab.
fn tabs(tabs: &str) -> String {
    let tabs: String = tabs.lines().map(|l| format!("      {l}\n")).collect();
    format!(
        "routes nav\n  tab home \"/\"\n  tab more \"/more\"\ncomponent App\n  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\"\n      each t in nav.tabs key=t.name\n        column role=\"tabpanel\" id=`panel-${{t.name}}`\n          each e in t.stack key=e.id\n            column navigationKey=`${{e.id}}`\n              text e.name\n      row role=\"tablist\"\n{tabs}"
    )
}

#[test]
fn a_tab_bar_of_one_tab_is_named_and_two_are_not() {
    let one = tabs(
        "  button role=\"tab\" aria-controls=\"panel-home\" aria-selected=true\n    text \"Home\"",
    );
    assert_eq!(warned(&one), [("lower-single-tab".to_string(), 12)]);
    let (_, warnings) = contract::compile_warned(&one);
    assert!(
        warnings[0].message.contains("has 1 tab")
            && warnings[0].message.contains("drop the `role=\"tablist\"`"),
        "{warnings:?}"
    );
    let two = tabs("  button role=\"tab\" aria-controls=\"panel-home\" aria-selected=true\n    text \"Home\"\n  button role=\"tab\" aria-controls=\"panel-more\" aria-selected=false\n    text \"More\"");
    assert_eq!(warned(&two), []);
    // Tabs an `each` makes may be many; a content tablist (no panels
    // named) is a segmented control, which one segment does not break.
    let made = tabs("  each t in nav.tabs key=t.name\n    button role=\"tab\" aria-controls=`panel-${t.name}` aria-selected=true\n      text t.name");
    assert_eq!(warned(&made), []);
    let segments = tabs("  button role=\"tab\" aria-selected=true\n    text \"Home\"");
    assert_eq!(warned(&segments), []);
}

/// A stopwatch's actions over a mutation, each `send` on line 16.
fn stopwatch(state: &str, send: &str) -> String {
    format!(
        "shape Time\n  epochAtZero: number\nshape Ack\n  ok: bool\nfn later(t: number): number = t + 10\nfn mark(): number = performanceNow()\ncomponent App\n  resource time = exactTime() as shape Time\n  mutation wrote as shape Ack\n  state start = 0\n  state other = 0\n{state}  action begin\n    start = performanceNow()\n  action save\n    send wrote = keep({send})\n  view\n    button \"save\" press=save\n"
    )
}

const PERSISTED: &str = "type-performance-now-persisted";

#[test]
fn a_launch_relative_time_sent_to_a_source_is_named_with_its_repair() {
    for send in [
        "performanceNow()",
        "start",
        "start + 1000",
        "floor(start / 1000)",
        "later(start)",
        "mark()",
        "`${start}`",
        "(other > 0 ? start : 0)",
        "max(start, 5)",
    ] {
        assert_eq!(
            warned(&stopwatch("", send)),
            [(PERSISTED.to_string(), 15)],
            "{send}"
        );
    }
    let (_, warnings) = contract::compile_warned(&stopwatch("", "start"));
    assert!(
        warnings[0].message.contains("this argument to `keep`")
            && warnings[0]
                .message
                .contains("`time.epochAtZero + performanceNow()`"),
        "{warnings:?}"
    );
    // Through a derive, and a `let` in the action.
    let derived = stopwatch("  derive since = start\n", "since");
    assert_eq!(warned(&derived)[0].0, PERSISTED);
    let local = stopwatch("", "t").replace(
        "    send wrote = keep(t)",
        "    let t = performanceNow()\n    send wrote = keep(t)",
    );
    assert_eq!(warned(&local), [(PERSISTED.to_string(), 16)]);
}

#[test]
fn the_wall_clock_time_and_a_duration_are_not_named() {
    for send in [
        "time.epochAtZero + performanceNow()",
        "performanceNow() + time.epochAtZero",
        "time.epochAtZero + start",
        "performanceNow() - start",
        "(performanceNow() - start) / 1000",
        "other",
        "length(`${start}`)",
        "toString(start > 5)",
    ] {
        assert_eq!(warned(&stopwatch("", send)), [], "{send}");
    }
    // A derive of the origin, added as `epochAtZero` is.
    let origin = stopwatch("  derive origin = time.epochAtZero\n", "origin + start");
    assert_eq!(warned(&origin), []);
}

#[test]
fn a_launch_relative_time_in_a_persisted_state_is_named() {
    // stopwatch's start mark, kept by `persist` instead of a source.
    let src = "shape Time\n  epochAtZero: number\ncomponent App\n  resource time = exactTime() as shape Time\n  state start = 0 persist\n  state wall = 0 persist\n  state lap = 0 persist\n  action begin\n    lap = performanceNow() - start\n    start = performanceNow()\n    wall = time.epochAtZero + performanceNow()\n  view\n    button \"go\" press=begin\n";
    assert_eq!(warned(src), [(PERSISTED.to_string(), 5)]);
}

const ONE_SCHEME: &str = "lower-colour-one-scheme";

/// One screen, its view's lines at 5 on, with a style, a `fn` and a derive
/// that choose colours.
fn screen(view: &str) -> String {
    let view: String = view.lines().map(|l| format!("    {l}\n")).collect();
    format!(
        "fn tone(n: number): string = n > 3 ? \"#e8f1ec\" : \"-exact-fill\"\ncomponent App\n  state n = 1\n  view\n{view}"
    )
}

#[test]
fn a_literal_colour_on_the_page_is_named_with_its_repair() {
    // Round 4's light card and its tinted text: the same in dark mode.
    let card = screen("column background-color=\"#ffffff\"\n  text \"a\"");
    assert_eq!(warned(&card), [(ONE_SCHEME.to_string(), 5)]);
    let (_, warnings) = contract::compile_warned(&card);
    let m = &warnings[0].message;
    assert!(
        m.contains("`#ffffff` on `background-color` stays the same in dark mode")
            && m.contains("`-exact-secondary-grouped-background`")
            && m.contains("one `light-dark(light, dark)` pair on `accent-color`"),
        "{m}"
    );
    let text = screen("text \"a\" color=\"rgb(20, 30, 40)\"");
    assert_eq!(warned(&text), [(ONE_SCHEME.to_string(), 5)]);
    let (_, warnings) = contract::compile_warned(&text);
    assert!(
        warnings[0]
            .message
            .contains("`rgb(20, 30, 40)` on `color` stays the same in dark mode")
            && warnings[0].message.contains("`-exact-label`"),
        "{warnings:?}"
    );
    // A forced scheme is the app with no dark mode: still named.
    for scheme in ["light", "dark"] {
        let forced = screen(&format!(
            "main color-scheme=\"{scheme}\" background-color=\"#fafafa\"\n  text \"a\""
        ));
        assert_eq!(warned(&forced), [(ONE_SCHEME.to_string(), 5)], "{scheme}");
    }
    // A named colour, `hsl()`, a border's colour, the bare accent, the
    // leaves of a choice, a `fn`'s and a derive's, and a style's rows.
    for view in [
        "text \"a\" color=\"white\"",
        "text \"a\" background-color=\"hsl(120 40% 50%)\"",
        "text \"a\" border-bottom=\"1px solid #dddddd\"",
        "column accent-color=\"#0a7f5a\"",
        "text \"a\" color=(n > 1 ? \"#111111\" : \"#222222\")",
        "text \"a\" background-color=tone(n)",
    ] {
        assert_eq!(
            warned(&screen(view)),
            [(ONE_SCHEME.to_string(), 5)],
            "{view}"
        );
    }
    let derived = screen("text \"a\" color=shade").replace(
        "  view\n",
        "  derive shade = n > 2 ? \"#333333\" : \"-exact-label\"\n  view\n",
    );
    assert_eq!(warned(&derived), [(ONE_SCHEME.to_string(), 6)]);
    let styled = screen("column class=Card").replace(
        "component App\n",
        "style Card\n  background-color=\"#fafafa\"\ncomponent App\n",
    );
    assert_eq!(warned(&styled), [(ONE_SCHEME.to_string(), 3)]);
}

#[test]
fn the_accent_pair_roles_and_artwork_are_not_named() {
    for view in [
        // The one authored colour, as a pair or a role.
        "main accent-color=\"light-dark(#0a7f5a, #3cc98f)\"\n  text \"a\"",
        "main accent-color=\"-exact-system-teal\"\n  text \"a\"",
        // Roles, and what is no colour of its own.
        "text \"a\" color=\"-exact-secondary-label\" background-color=\"-exact-secondary-grouped-background\" border-color=\"-exact-separator\"",
        "text \"a\" color=\"CanvasText\" background-color=\"AccentColor\" -exact-tint-color=\"-exact-system-red\"",
        "text \"a\" color=\"currentColor\" background-color=\"transparent\" border-color=\"rgba(0, 0, 0, 0)\"",
        "text \"a\" color=\"-exact-platform-color(ios systemPinkColor, #ff2d55)\"",
        // Artwork: an `svg`'s paint, and a shadow, which darkens in both.
        "svg viewBox=\"0 0 10 10\" width=10 height=10\n  circle cx=5 cy=5 r=4 fill=\"#ff0000\" stroke=\"#000000\"",
        "text \"a\" box-shadow=\"0 1px 2px rgba(0, 0, 0, 0.2)\"",
        // What the compiler cannot see: a state, an item, a field.
        "column\n  each c in [\"#ffffff\"] key=c\n    text c color=c",
    ] {
        assert_eq!(warned(&screen(view)), [], "{view}");
    }
}

#[test]
fn every_literal_colour_is_named_past_the_cap_with_one_line_on_why() {
    let view: String = (0..25)
        .map(|i| format!("  text \"{i}\" color=\"#1234{i:02}\"\n"))
        .collect();
    let view = format!("column\n{view}");
    let all = warned(&screen(&view));
    assert_eq!(all.len(), 26, "{all:?}");
    assert!(all.iter().all(|(id, _)| id == ONE_SCHEME), "{all:?}");
    let (_, warnings) = contract::compile_warned(&screen(&view));
    assert!(
        warnings[0].message.ends_with("or leave it unsaid")
            && warnings[25].message.starts_with("these 25 literal colours")
            && warnings[25].span.line == 6,
        "{warnings:?}"
    );
}

const REREAD: &str = "analyze-read-after-assign";

/// One action, `go`, its body from line 10 (LLP 1116.003 D6).
fn go(body: &str) -> String {
    let body: String = body.lines().map(|l| format!("    {l}\n")).collect();
    format!(
        "shape Ack\n  ok: bool\ncomponent App\n  mutation saved as shape Ack\n  state count = 0\n  state doubled = 0\n  state mode = \"a\"\n  state shown = \"\"\n  action go\n{body}  view\n    button \"go\" press=go\n"
    )
}

#[test]
fn a_read_of_a_slot_its_action_assigned_is_named_with_its_repair() {
    // The doc's example, written the way focus-timer and
    // password-generator wrote it: the read sees the starting `count`.
    let direct = go("count = count + 1\ndoubled = count * 2");
    assert_eq!(warned(&direct), [(REREAD.to_string(), 11)]);
    let (_, warnings) = contract::compile_warned(&direct);
    assert_eq!(
        warnings[0].message,
        "`count` here is its value from before the action, not the one assigned at line 10: writes land when the action ends. Bind the new value and read it: `let next = …`, `count = next`, then `next`"
    );
    // In a later branch's condition and its `let`, on the path that
    // assigned it.
    let nested = go(
        "count = count + 1\nif mode == \"a\"\n  if count > 3\n    let seen = count\n    shown = `${seen}`",
    );
    assert_eq!(
        warned(&nested),
        [(REREAD.to_string(), 12), (REREAD.to_string(), 13)]
    );
    // A `send`'s argument; an assignment in one arm, read after the branch.
    let sent = go("count = count + 1\nsend saved = keep(count)");
    assert_eq!(warned(&sent), [(REREAD.to_string(), 11)]);
    let arm = go("if mode == \"a\"\n  count = 1\nshown = `${count}`");
    assert_eq!(warned(&arm), [(REREAD.to_string(), 12)]);
}

#[test]
fn a_read_before_the_assignment_on_another_path_or_of_a_let_is_not_named() {
    for (case, body) in [
        // An assignment reads the starting value it replaces.
        ("before", "doubled = count * 2\ncount = count + 1"),
        // The doc's repair.
        ("let", "let next = count + 1\ncount = next\ndoubled = next * 2"),
        ("else", "if mode == \"a\"\n  count = 1\nelse\n  doubled = count"),
        // Sequential arms over one name and literals no evaluation can
        // both satisfy; the right of an `and` runs where its left held.
        (
            "exclusive",
            "if mode == \"a\"\n  count = 1\nif mode == \"b\"\n  doubled = count\nif mode == \"b\" and count > 0\n  shown = \"b\"",
        ),
        // A chain over the starting state steps once an action.
        (
            "steps",
            "if mode == \"a\"\n  mode = \"b\"\nif mode == \"b\"\n  mode = \"c\"",
        ),
    ] {
        assert_eq!(warned(&go(body)), [], "{case}");
    }
}

#[test]
fn a_field_the_copy_left_alone_is_not_named() {
    // fieldnotes' `session = Session(session, savedId=…)`, then
    // `session.version` in the `send`: the same value either way.
    let src = "shape Ack\n  ok: bool\nshape Item\n  id: string\n  n: number\ncomponent App\n  mutation saved as shape Ack\n  state item = Item(id=\"x\", n=0)\n  state shown = \"\"\n  action go\n    item = Item(item, n=item.n + 1)\n    send saved = keep(item.id)\n    shown = `${item.n}`\n  view\n    button \"go\" press=go\n";
    assert_eq!(warned(src), [(REREAD.to_string(), 13)]);
}
