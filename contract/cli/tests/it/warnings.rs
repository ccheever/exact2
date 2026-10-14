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
        // A heading under a `when` may be the one: the benefit.
        "  when q != \"\"\n    text q role=\"heading\"\n  text \"sub\"",
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

const NO_BAR: &str = "lower-header-no-bar";

#[test]
fn a_header_with_no_heading_or_two_is_named_as_no_bar() {
    // No heading, or two: on iOS the header is no bar and is drawn as
    // written; nothing in it is called unplaced (LLP 1116.003 D2.2). The
    // bar it leaves is the system Back alone on a pushed route (LLP
    // 1115.000 D5.2), and the message names the way to have none.
    let none = route("  text \"one\"\n  button press=go testId=\"add\"\n    text \"Add\"");
    assert_eq!(warned(&none), [(NO_BAR.to_string(), 11)]);
    let (_, warnings) = contract::compile_warned(&none);
    assert!(
        warnings[0].message.contains("holds no heading")
            && warnings[0]
                .message
                .contains("holds the system Back button alone")
            && warnings[0].message.contains("one `text role=\"heading\"`")
            && warnings[0].message.contains("`navigationBar=\"none\"`"),
        "{warnings:?}"
    );
    // A route that says it has no bar draws its header as written: nothing
    // to warn of, whatever the header holds.
    let said = none.replace(
        "column navigationKey=`${e.id}`",
        "column navigationKey=`${e.id}` navigationBar=\"none\"",
    );
    assert_eq!(warned(&said), []);
    let two = route("  text \"A\" role=\"heading\" aria-level=1\n  text \"B\" role=\"heading\" aria-level=2\n  image \"symbol:heart\"");
    assert_eq!(warned(&two), [(NO_BAR.to_string(), 11)]);
    let (_, warnings) = contract::compile_warned(&two);
    assert!(
        warnings[0].message.contains("holds 2 headings")
            && warnings[0].message.contains("make the others plain `text`"),
        "{warnings:?}"
    );
    // A route that opens with `when`, as workout-log's did: each branch's
    // header is the route's first child while its branch is taken.
    let branched = route("  text \"List\" role=\"heading\" aria-level=1").replace(
        "          header display=\"flex\"\n            text \"List\" role=\"heading\" aria-level=1\n",
        "          when q == \"\"\n            header\n              text \"Sheet\"\n          else\n            header\n              text \"List\" role=\"heading\" aria-level=1\n              image \"symbol:heart\"\n",
    );
    assert_eq!(
        warned(&branched),
        [(NO_BAR.to_string(), 12), (UNPLACED.to_string(), 17)],
        "{branched}"
    );
}

/// A stack whose root names its Back control `back` (or `root` says
/// otherwise), with a route pushed over Home: `route` is the pushed route's
/// attributes and `body` its children, their lines at 16 on.
fn pushed(root: &str, route: &str, body: &str) -> String {
    let body: String = body.lines().map(|l| format!("            {l}\n")).collect();
    format!(
        "routes nav\n  home \"/\"\n    detail \"/detail\"\ncomponent App\n  action back\n    nav = back(nav)\n  view\n    main navigationKey=`${{top(nav).id}}` {root}\n      each e in stack(nav) key=e.id\n        when e.name == \"home\"\n          column navigationKey=`${{e.id}}`\n            header\n              text \"Home\" role=\"heading\" aria-level=1\n        when e.name == \"detail\"\n          column navigationKey=`${{e.id}}` {route}\n{body}"
    )
}

const BACK_TWICE: &str = "lower-back-twice";
const NAMED: &str = "navigationBack=\"back\"";
const BACK: &str = "button id=\"back\" press=back\n  text \"Back\"";

#[test]
fn a_back_control_drawn_under_the_system_back_is_named_with_the_opt_out() {
    // LLP 1115.000 D5.2: on iOS every pushed route has the bar, whose Back
    // presses the authored control, so one drawn in the page is a second.
    let drawn = pushed(NAMED, "", &format!("{BACK}\ntext \"Detail\""));
    assert_eq!(warned(&drawn), [(BACK_TWICE.to_string(), 16)], "{drawn}");
    let (_, warnings) = contract::compile_warned(&drawn);
    for part in [
        "`id=\"back\"`",
        "the screen shows two",
        "Delete it",
        "into the route's `header`",
        "`navigationBar=\"none\"`",
    ] {
        assert!(warnings[0].message.contains(part), "{part}: {warnings:?}");
    }
    // Under a header that is the bar, too: the bar's Back and this one.
    let below = pushed(
        NAMED,
        "",
        &format!("header\n  text \"Detail\" role=\"heading\"\n{BACK}"),
    );
    assert_eq!(warned(&below), [(BACK_TWICE.to_string(), 18)], "{below}");
    // And in a `header` that is not the route's first child, which is a
    // box in the page.
    let second = pushed(
        NAMED,
        "",
        &format!("text \"first\"\nheader\n  {}", BACK.replace('\n', "\n  ")),
    );
    assert_eq!(warned(&second), [(BACK_TWICE.to_string(), 18)], "{second}");
}

#[test]
fn a_back_control_the_bar_stands_for_or_a_route_with_no_bar_is_not_warned() {
    let in_header = format!(
        "header\n  {}\n  text \"Detail\" role=\"heading\"\ntext \"body\"",
        BACK.replace('\n', "\n  ")
    );
    for (root, route, body) in [
        // In the route's first `header`, which is the bar: UIKit's Back
        // button stands for it, and it is not drawn.
        (NAMED, "", in_header.clone()),
        // The route says it has no bar: its own Back is the only one.
        (NAMED, "navigationBar=\"none\"", BACK.to_string()),
        // A sheet is the first route of its own stack: no system Back.
        (NAMED, "navigationPresentation=\"modal\"", BACK.to_string()),
        (
            NAMED,
            "navigationPresentation=\"fullscreen\"",
            BACK.to_string(),
        ),
        // What the compiler cannot read is given the benefit.
        (
            NAMED,
            "navigationBar=(e.name == \"detail\" ? \"none\" : \"auto\")",
            BACK.to_string(),
        ),
        (
            NAMED,
            "navigationPresentation=(e.name == \"detail\" ? \"modal\" : \"\")",
            BACK.to_string(),
        ),
        (
            NAMED,
            "",
            "button id=`${e.name}` press=back\n  text \"Back\"".to_string(),
        ),
        // Not drawn in the page: hidden, or in a popover.
        (
            NAMED,
            "",
            "button id=\"back\" press=back display=\"none\"\n  text \"Back\"".to_string(),
        ),
        (
            NAMED,
            "",
            format!(
                "column id=\"more\" popover=\"auto\"\n  {}",
                BACK.replace('\n', "\n  ")
            ),
        ),
        // A root that names no Back control: `id="back"` is any element.
        ("", "", BACK.to_string()),
        // One element for every route, each branch opening with its bar:
        // a later branch's header is the route's first child when taken.
        (
            NAMED,
            "",
            format!(
                "when e.url == \"/\"\n  header\n    text \"A\" role=\"heading\"\nwhen e.url != \"/\"\n  {}",
                in_header.replace('\n', "\n  ")
            ),
        ),
    ] {
        let src = pushed(root, route, &body);
        assert_eq!(warned(&src), [], "{src}");
    }
    // A written `auto` or an empty presentation is the default, and warned.
    for route in ["navigationBar=\"auto\"", "navigationPresentation=\"\""] {
        let src = pushed(NAMED, route, BACK);
        assert_eq!(warned(&src), [(BACK_TWICE.to_string(), 16)], "{src}");
    }
    // In a first `header` that is no bar, the header's own warning names
    // the bar that holds the system Back above it; the control is not
    // named a second time.
    let no_bar = pushed(
        NAMED,
        "",
        &format!("header\n  {}", BACK.replace('\n', "\n  ")),
    );
    assert_eq!(warned(&no_bar), [(NO_BAR.to_string(), 16)], "{no_bar}");
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

const UNLABELLED: &str = "lower-button-unlabelled";

/// One screen of buttons, its view's lines at 6 on, with a `Pill`
/// component whose button is filled through its slot.
fn buttons(view: &str) -> String {
    let view: String = view.lines().map(|l| format!("      {l}\n")).collect();
    format!(
        "component App\n  state label = \"Add\"\n  action go\n    label = \"x\"\n  view\n    column\n{view}component Pill\n  slot\n  view\n    button width=44 height=44\n      children\n"
    )
}

#[test]
fn a_button_with_nothing_to_say_is_named_with_its_repair() {
    // water-tracker's +500 ml button: no child, blank on every host.
    let blank = buttons("button press=go testId=\"add-500\" flex=1");
    assert_eq!(warned(&blank), [(UNLABELLED.to_string(), 7)]);
    let (_, warnings) = contract::compile_warned(&blank);
    assert!(
        warnings[0]
            .message
            .contains("holds no text and no `aria-label`")
            && warnings[0].message.contains("Put its words in it")
            && warnings[0].message.contains("docs/recipes/symbols.md"),
        "{warnings:?}"
    );
    // A symbol with no name is drawn and read as nothing; a swatch, an
    // empty text and an `svg` of shapes say nothing either.
    let symbol = buttons("button press=go appearance=\"none\"\n  image \"symbol:add\"");
    assert_eq!(warned(&symbol), [(UNLABELLED.to_string(), 7)]);
    let (_, warnings) = contract::compile_warned(&symbol);
    assert!(
        warnings[0]
            .message
            .contains("shows only an image with no name")
            && warnings[0]
                .message
                .contains("`aria-label=\"Add\"` on the button"),
        "{warnings:?}"
    );
    for view in [
        "button press=go\n  row\n    view width=10 height=10 background-color=\"-exact-system-red\"",
        "button press=go aria-label=\"\"\n  text \"\"",
        "button press=go\n  svg viewBox=\"0 0 10 10\" width=10 height=10\n    circle cx=5 cy=5 r=4",
        "button press=go\n  when label == \"\"\n    image \"symbol:add\"",
    ] {
        assert_eq!(
            warned(&buttons(view)),
            [(UNLABELLED.to_string(), 7)],
            "{view}"
        );
    }
    // A component used twice is one line, at its button; an empty slot
    // fill leaves it blank.
    let pills = buttons("Pill()\nPill()");
    assert_eq!(warned(&pills), [(UNLABELLED.to_string(), 12)]);
}

#[test]
fn a_button_with_words_a_name_or_a_doubt_is_not_named() {
    for view in [
        "button press=go\n  text \"Add\"",
        // A text of any expression, through a `when` or an `each`.
        "button press=go\n  text label",
        "button press=go\n  when label == \"\"\n    text \"Add\"\n  else\n    image \"symbol:add\"",
        "button press=go\n  each w in [\"a\"] key=w\n    text w",
        // An icon button named, by itself or by its image.
        "button press=go aria-label=\"Add\"\n  image \"symbol:add\"",
        "button press=go aria-label=label\n  image \"symbol:add\"",
        "button press=go aria-labelledby=\"caption\" width=44 height=44",
        "button press=go\n  image \"symbol:add\" alt=\"Add\"",
        "button press=go\n  svg viewBox=\"0 0 10 10\" width=10 height=10 aria-label=\"Add\"\n    circle cx=5 cy=5 r=4",
        // A bare box keeps its words; one hidden from VoiceOver is no button to it.
        "button appearance=\"none\" press=go\n  row\n    text \"Add\"",
        "button press=go aria-hidden=true position=\"absolute\" inset=0",
        // Filled through a slot.
        "Pill()\n  text \"Add\"",
    ] {
        assert_eq!(warned(&buttons(view)), [], "{view}");
    }
}

const DIALOG_ROLE: &str = "lower-dialog-role";

#[test]
fn a_dialog_role_on_a_box_is_named_with_its_repair() {
    // flashcards' editor: a `column role="dialog" aria-modal=true`, with
    // no Escape, no focus kept inside and no iOS sheet.
    let editor = buttons(
        "column role=\"dialog\" aria-modal=true aria-label=\"Add a flashcard\" testId=\"editor\"\n  text \"Add a flashcard\" role=\"heading\"\n  button press=go\n    text \"Save\"",
    );
    assert_eq!(warned(&editor), [(DIALOG_ROLE.to_string(), 7)]);
    let (_, warnings) = contract::compile_warned(&editor);
    let m = &warnings[0].message;
    assert!(
        m.starts_with("`role=\"dialog\"` on a `column` names a dialog without making one")
            && m.contains("no Escape")
            && m.contains("a route with `navigationPresentation=\"modal\"`")
            && m.contains("`dialog role=\"alertdialog\"`, opened with `showModal(id)`")
            && m.contains("docs/recipes/navigation-stack.md"),
        "{m}"
    );
    let alert =
        buttons("view role=\"alertdialog\" aria-label=\"Delete?\"\n  text \"Gone for good.\"");
    assert_eq!(warned(&alert), [(DIALOG_ROLE.to_string(), 7)]);
}

#[test]
fn a_dialog_a_popover_and_a_modal_route_are_not_named() {
    for view in [
        // The `dialog` element, as a confirmation or laid out.
        "dialog id=\"remove\" role=\"alertdialog\" aria-label=\"Delete?\"\n  text \"Gone for good.\"\n  button press=go commandfor=\"remove\" command=\"close\"\n    text \"Delete\"\n  button commandfor=\"remove\" command=\"close\"\n    text \"Cancel\"",
        "dialog id=\"edit\" role=\"dialog\" closedby=\"any\"\n  text \"Edit\"",
        // A popover: light dismiss and Escape; as an alertdialog, the
        // native confirmation.
        "column id=\"pick\" popover=\"auto\" role=\"dialog\" aria-label=\"Theme\"\n  text \"Light\"",
        // A computed role is the benefit of the doubt.
        "column role=(label == \"\" ? \"dialog\" : \"region\")\n  text \"a\"",
        // `aria-modal` alone keeps VoiceOver inside an overlay (LLP 1080.003).
        "column aria-modal=true\n  text \"Call\"",
    ] {
        assert_eq!(warned(&buttons(view)), [], "{view}");
    }
    // A route the router presents as a sheet.
    let sheet = "routes nav\n  home \"/\"\n    add \"/add\"\ncomponent App\n  action back\n    nav = back(nav)\n  view\n    main navigationKey=`${top(nav).id}` navigationBack=\"back\" width=\"100%\" height=\"100%\"\n      each e in stack(nav) key=e.id\n        column navigationKey=`${e.id}` navigationPresentation=(e.name == \"add\" ? \"modal\" : \"push\") role=\"dialog\" position=\"absolute\" inset=0\n          text e.name\n";
    assert_eq!(warned(sheet), []);
}

const SYMBOL_ALIAS: &str = "lower-symbol-alias";

// @ref LLP 1116.007 D3.1 — a role also answers to Apple's name for it (dotted, dashed or
// underscored) and to the schema's aliases: the plan carries the role, and a warning names it.
// Round 9: 12 of 24 builders guessed one of these names and rebuilt.
#[test]
fn symbols_answer_to_apple_names_and_aliases_and_the_plan_carries_the_role() {
    for (written, role) in [
        ("gearshape", "settings"),
        ("gear", "settings"),
        ("trash", "delete"),
        ("plus", "add"),
        ("xmark", "close"),
        ("chevron.right", "forward-chevron"),
        ("chevron-right", "forward-chevron"),
        ("chevron.forward", "forward-chevron"),
        ("chevron_left", "back"),
        ("checkmark.circle.fill", "select-fill"),
        ("checkmark-circle-fill", "select-fill"),
        ("checkmark.circle", "select"),
        ("square.and.arrow.up", "share"),
        ("magnifyingglass", "search"),
        ("bell", "notifications"),
        ("bell.fill", "notifications-fill"),
        ("house", "home"),
        ("house.fill", "home-fill"),
        ("star.fill", "star-fill"),
        ("sign-out", "sign-out"),
    ] {
        let (plan, warnings) = contract::compile_warned(&format!(
            "component App\n  view\n    image \"symbol:{written}\"\n"
        ));
        let plan = plan.unwrap_or_else(|e| panic!("{written}: {e}"));
        let has = |s: &str| plan.strings.iter().any(|x| x == s);
        assert!(
            has(&format!("symbol:{role}")),
            "{written}: {:?}",
            plan.strings
        );
        assert!(
            written == role || !has(&format!("symbol:{written}")),
            "{written}"
        );
        // The warning is the role, written; the role itself is silent.
        let said: Vec<_> = warnings
            .iter()
            .map(|w| (w.id.as_str(), w.message.as_str()))
            .collect();
        if written == role {
            assert!(said.is_empty(), "{said:?}");
        } else {
            let message = format!("symbol `{written}` is the role `{role}`: write `symbol:{role}`");
            assert_eq!(said, [("lower-symbol-alias", message.as_str())]);
            assert!(
                warnings[0].is_warning() && warnings[0].span.line == 3,
                "{:?}",
                warnings[0]
            );
        }
    }
    // Each arm of a choice is its role, and warns once; an unknown name in an arm is refused.
    let (plan, warnings) = contract::compile_warned(
        "component App\n  state on = false\n  view\n    button press=toggle aria-label=\"Mark\"\n      image (on ? \"symbol:checkmark.circle.fill\" : \"symbol:circle\")\n  action toggle\n    on = not on\n",
    );
    assert!(plan
        .unwrap()
        .strings
        .iter()
        .any(|s| s == "symbol:select-fill"));
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let error = contract::compile(
        "component App\n  state on = false\n  view\n    image (on ? \"symbol:gearshape\" : \"symbol:gearsh\")\n",
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("symbol `gearsh` is not a role"),
        "{error}"
    );
    // An alias keeps the distinctions the roles make: a symbol no role draws is no alias.
    for name in [
        "lock.fill",
        "trash.fill",
        "exclamationmark.triangle",
        "user",
        "check-circle",
        "paperplane.fill",
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    image \"symbol:{name}\"\n"
        ))
        .unwrap_err();
        assert!(error.to_string().contains("not a role"), "{name}: {error}");
    }
    // `sf/` stays distinct: Apple's name, opaque, and no warning.
    let (plan, warnings) =
        contract::compile_warned("component App\n  view\n    image \"symbol:sf/gearshape\"\n");
    assert!(plan
        .unwrap()
        .strings
        .iter()
        .any(|s| s == "symbol:sf/gearshape"));
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn a_symbol_by_another_spelling_than_its_role_is_named_with_the_role() {
    // LLP 1116.007 D3.1: `symbol:trash` builds as the role `delete`, and says so.
    // A component used twice is one line; each literal arm of a choice is its own.
    let src = "component App\n  state on = false\n  view\n    column\n      Chip()\n      Chip()\n      image (on ? \"symbol:gearshape\" : \"symbol:settings\")\n      image \"symbol:sf/trash\"\n      image \"symbol:delete\"\n      image \"symbol:checkmark.circle.fill\"\ncomponent Chip\n  view\n    image \"symbol:trash\" width=16\n";
    assert_eq!(
        warned(src),
        [
            (SYMBOL_ALIAS.to_string(), 13),
            (SYMBOL_ALIAS.to_string(), 7),
            (SYMBOL_ALIAS.to_string(), 10)
        ]
    );
    let (_, warnings) = contract::compile_warned(src);
    assert_eq!(
        warnings[0].message,
        "symbol `trash` is the role `delete`: write `symbol:delete`"
    );
    assert_eq!(
        warnings[2].message,
        "symbol `checkmark.circle.fill` is the role `select-fill`: write `symbol:select-fill`"
    );
}

const LEVEL: &str = "lower-level-without-heading";

#[test]
fn a_level_with_no_heading_role_is_named_with_its_repair() {
    // LLP 1115.000 D12.5: `aria-level` alone is not a heading. No host tells
    // a screen reader it is one, it gets no heading style, and the iOS bar
    // does not take it for the title.
    let bare =
        buttons("text \"Results\" aria-level=2\ntext \"More\" aria-level=(label == \"\" ? 2 : 3)");
    assert_eq!(
        warned(&bare),
        [(LEVEL.to_string(), 7), (LEVEL.to_string(), 8)]
    );
    let (_, warnings) = contract::compile_warned(&bare);
    let m = &warnings[0].message;
    assert!(
        m.starts_with(
            "`aria-level` on a `text` with no `role=\"heading\"` does not make a heading"
        ) && m.contains("a screen reader is not told it is one on any platform")
            && m.contains("no heading style")
            && m.contains("the iOS navigation bar does not take it for the screen's title")
            && m.contains("`text \"Results\" role=\"heading\" aria-level=2`"),
        "{m}"
    );
    // In a route's header it is not the bar's title either: the header
    // holds no heading, which the bar's own warning says beside this one.
    let header = route("  text \"List\" aria-level=1");
    assert_eq!(
        warned(&header),
        [
            ("lower-header-no-bar".to_string(), 11),
            (LEVEL.to_string(), 12)
        ]
    );
    let (_, warnings) = contract::compile_warned(&header);
    assert!(
        warnings[0].message.contains("holds no heading"),
        "{warnings:?}"
    );
}

#[test]
fn a_level_beside_a_role_is_not_named() {
    for view in [
        "text \"Results\" role=\"heading\" aria-level=2",
        "text \"Results\" aria-level=2 role=\"heading\"",
        // A heading with no level is a level-2 heading.
        "text \"Results\" role=\"heading\"",
        // A computed role is the benefit of the doubt.
        "text \"Results\" role=(label == \"\" ? \"heading\" : \"none\") aria-level=2",
        // A tree's item takes a level of its own.
        "row role=\"treeitem\" aria-level=2\n  text \"src\"",
        "text \"Results\"",
    ] {
        assert_eq!(warned(&buttons(view)), [], "{view}");
    }
    assert_eq!(
        warned(&route("  text \"List\" role=\"heading\" aria-level=1")),
        []
    );
}

const COUNT_PLURAL: &str = "lower-count-plural";

/// One screen over a count and a name, its view's lines at 9 on, with a
/// `fn` on line 1 for the cases that need one.
fn counted(view: &str, function: &str) -> String {
    let view: String = view.lines().map(|l| format!("      {l}\n")).collect();
    format!(
        "{function}\nshape Event\n  going: number\n  capacity: number\ncomponent App\n  state known = 1\n  state total = 3\n  state name = \"Ada\"\n  state events = [Event(going=3, capacity=4)]\n  view\n    column\n{view}"
    )
}

#[test]
fn a_count_before_a_plural_noun_is_named_with_its_repair() {
    // flashcards' and event-rsvp's round-6 lines: the noun fixed, the count
    // free to be 1, under a choice made on something else.
    let cards = counted(
        "text (known == total ? \"You worked through every card.\" : `${total - known} cards to keep practicing next time.`)",
        "",
    );
    assert_eq!(warned(&cards), [(COUNT_PLURAL.to_string(), 12)]);
    let (_, warnings) = contract::compile_warned(&cards);
    assert!(
        warnings[0]
            .message
            .starts_with("this count reads \"1 cards\" when it is 1")
            && warnings[0]
                .message
                .contains("`${n} ${n == 1 ? \"card\" : \"cards\"}`")
            && warnings[0].message.contains("`contract vocab plural`"),
        "{warnings:?}"
    );
    let spots = counted(
        "each e in events key=e.going\n  text (e.going >= e.capacity ? \"Full · no spots left\" : `${e.going} going · ${e.capacity - e.going} spots left`)",
        "",
    );
    assert_eq!(warned(&spots), [(COUNT_PLURAL.to_string(), 13)]);
    // An irregular plural, a `length`, a derive, a `fn`'s parameter, an
    // action's message.
    for (view, function) in [
        ("text `${total} people`", ""),
        ("text `${length(events)} events`", ""),
        (
            "text daysText(total)",
            "fn daysText(d: number): string = `${d} days`",
        ),
    ] {
        let src = counted(view, function);
        assert_eq!(warned(&src).len(), 1, "{src}");
        assert_eq!(warned(&src)[0].0, COUNT_PLURAL, "{src}");
    }
    let (_, warnings) = contract::compile_warned(&counted("text `${total} people`", ""));
    assert!(
        warnings[0]
            .message
            .contains("`${n} ${n == 1 ? \"person\" : \"people\"}`"),
        "{warnings:?}"
    );
    let toast = counted("text name", "").replace(
        "  view\n",
        "  action add\n    let n = total + 1\n    name = `${n} entries saved`\n  view\n",
    );
    assert_eq!(warned(&toast), [(COUNT_PLURAL.to_string(), 12)]);
}

#[test]
fn a_chosen_noun_a_string_a_unit_or_a_total_is_not_named() {
    for (view, function) in [
        // The noun chosen by the number, in the template or around it.
        (
            "text `${total} ${total == 1 ? \"card\" : \"cards\"} left`",
            "",
        ),
        ("text (total == 1 ? \"1 card\" : `${total} cards`)", ""),
        ("text (total != 1 ? `${total} cards` : \"1 card\")", ""),
        ("when total > 1\n  text `${total} cards`", ""),
        (
            "text cardsText(total)",
            "fn cardsText(n: number): string = n == 1 ? \"1 card\" : `${n} cards`",
        ),
        // A string, a unit's abbreviation, a percent, a word that is no
        // plural, a total after `of`.
        ("text `${name} votes`", ""),
        ("text `${total} lbs · ${total} ms · ${total} kg`", ""),
        ("text `${total}% done`", ""),
        ("text `${total} plus tax, ${total} was`", ""),
        ("text `${known} of ${total} cards`", ""),
    ] {
        let src = counted(view, function);
        assert_eq!(warned(&src), [], "{src}");
    }
}

const TITLE_TWICE: &str = "lower-title-twice";

/// A route whose header holds `title` and whose scroller holds `content`,
/// its lines at 14 on after a header of one line.
fn titled(title: &str, content: &str) -> String {
    let content: String = content
        .lines()
        .map(|l| format!("            {l}\n"))
        .collect();
    route(title).replace("            text \"body\"\n", &content)
}

#[test]
fn a_content_heading_that_says_the_bars_title_is_named_with_its_repair() {
    // Trivia's home, rounds 6 and 7: "Trivia Quiz" large in the bar and
    // again at the top of the page, under the phase it shows in.
    let trivia = titled(
        "  text \"Trivia Quiz\" role=\"heading\" aria-level=1\n  when q != \"\"\n    text `Best ${q}`",
        "column\n  when q == \"\"\n    column align-items=\"center\"\n      text \"Trivia Quiz\" role=\"heading\" aria-level=2\n      text \"10 questions, 15 seconds each.\"\n  when q != \"\"\n    text \"Results\" role=\"heading\" aria-level=2",
    );
    assert_eq!(warned(&trivia), [(TITLE_TWICE.to_string(), 19)], "{trivia}");
    let (_, warnings) = contract::compile_warned(&trivia);
    let m = &warnings[0].message;
    assert!(
        m.starts_with("this heading says \"Trivia Quiz\", as the route's `header` heading does")
            && m.contains("on iOS once large in the navigation bar and once in the page")
            && m.contains("on the web just under the header")
            && m.contains("Drop this heading, or make it say something else"),
        "{m}"
    );
    // The same bound value in both, and the same words spaced apart.
    let bound = titled(
        "  text q role=\"heading\"",
        "text q role=\"heading\" aria-level=2",
    );
    assert_eq!(warned(&bound), [(TITLE_TWICE.to_string(), 14)], "{bound}");
    let (_, warnings) = contract::compile_warned(&bound);
    assert!(
        warnings[0].message.contains("says the same value")
            && warnings[0]
                .message
                .contains("on iOS once in the navigation bar"),
        "{warnings:?}"
    );
    let spaced = titled(
        "  text \"Trivia Quiz\" role=\"heading\" aria-level=1",
        "text \" Trivia  Quiz\" role=\"heading\" aria-level=2",
    );
    assert_eq!(warned(&spaced), [(TITLE_TWICE.to_string(), 14)]);
    // A level with no role is not a heading (LLP 1115.000 D12.5): it is
    // named for that, and not as a second title.
    let bare = titled(
        "  text \"Trivia Quiz\" role=\"heading\" aria-level=1",
        "text \"Trivia Quiz\" aria-level=2",
    );
    assert_eq!(warned(&bare), [(LEVEL.to_string(), 14)]);
}

#[test]
fn another_name_no_bar_or_a_sections_own_name_is_not_named() {
    let title = "  text \"Notes\" role=\"heading\" aria-level=1";
    for content in [
        // A heading that says something else.
        "text \"Recent\" role=\"heading\" aria-level=2\ntext \"Notes, newest first\"",
        // A section's own name: inside a `section`, or after a heading
        // always shown before it (expense-tracker's "Expenses" list under
        // "By category").
        "list\n  section\n    header\n      text \"Notes\" role=\"heading\" aria-level=3\n    text \"a\"",
        "column\n  text \"By folder\" role=\"heading\" aria-level=2\ncolumn\n  text \"Notes\" role=\"heading\" aria-level=2",
        // Not shown with the bar: hidden, in a popover or another route.
        "text \"Notes\" role=\"heading\" display=\"none\"\ncolumn id=\"m\" popover=\"auto\"\n  text \"Notes\" role=\"heading\"",
        // A plain text that says it is no heading.
        "text \"Notes\"",
    ] {
        let src = titled(title, content);
        assert_eq!(warned(&src), [], "{src}");
    }
    // A bound value under an `each` may name another value.
    let each = titled(
        "  text q role=\"heading\"",
        "each q in [\"a\"] key=q\n  text q role=\"heading\"",
    );
    assert_eq!(warned(&each), [], "{each}");
    // A header that is no bar: the title is drawn once, in the page.
    let no_bar = titled("  text \"Notes\"", "text \"Notes\" role=\"heading\"");
    assert_eq!(warned(&no_bar), [(NO_BAR.to_string(), 11)], "{no_bar}");
    let late = titled(title, "text \"Notes\" role=\"heading\"").replace(
        "          header display=\"flex\"\n",
        "          text \"first\"\n          header display=\"flex\"\n",
    );
    assert_eq!(warned(&late), [], "{late}");
}

const SCREEN: &str = "lower-screen-by-state";

/// The line `needle` is on, from 1.
fn line_of(src: &str, needle: &str) -> u32 {
    let at = src
        .find(needle)
        .unwrap_or_else(|| panic!("{needle}\n{src}"));
    src[..at].matches('\n').count() as u32 + 1
}

/// recipe-finder's round-7 page: its screens chosen in `choice`, inside one
/// `scroll` under a bar laid out by hand, with no `header` (LLP 1116.005).
fn finder(choice: &str) -> String {
    let choice: String = choice.lines().map(|l| format!("        {l}\n")).collect();
    format!(
        "shape Meal\n  id: string\n  name: string\ncomponent App\n  state query = \"\"\n  state chosen = []\n  state shared = \"\"\n  state tab = \"all\"\n  state open2 = false\n  resource meals = searchMeals(query) as shape list<Meal> else empty()\n  resource viewport = exactViewport() as shape Meal else empty()\n  derive showingDetail = length(chosen) > 0\n  action edit(v: string)\n    query = v\n  action open(m: Meal)\n    chosen = [m]\n  action back\n    chosen = []\n  action share(m: Meal)\n    shared = m.name\n  action pick(t: string)\n    tab = t\n    chosen = []\n  action toggle\n    open2 = not open2\n  view\n    column height=\"100%\" display=\"flex\" flex-direction=\"column\"\n      row padding=16\n        text \"Recipe Finder\" font-weight=700\n      scroll flex=1 min-height=0\n{choice}"
    )
}

/// The detail: Back, its heading deep in its layout, one more control.
const DETAIL: &str = "  column gap=16\n    button press=back testId=\"back\"\n      text \"Back to recipes\"\n    row gap=16\n      column\n        text m.name role=\"heading\" aria-level=1\n    button press=share(m) testId=\"share\"\n      text \"Share\"\n";
/// The list: its heading, a search field and rows that open the detail.
const LIST: &str = "  column gap=16\n    text \"What sounds good today?\" role=\"heading\" aria-level=1\n    input value=query input=edit type=\"search\" aria-label=\"Search\"\n    each m in meals key=m.id\n      button press=open(m) testId=`meal-${m.id}`\n        text m.name\n";

#[test]
fn screens_swapped_by_state_without_routes_are_named_with_the_repair() {
    // recipe-finder: Back left the app, a reload lost the recipe.
    let when = format!(
        "when showingDetail\n  each m in chosen key=m.id\n{}else\n{LIST}",
        DETAIL
            .replace("\n  ", "\n    ")
            .replacen("  column", "    column", 1)
    );
    let src = finder(&when);
    assert_eq!(
        warned(&src),
        [(SCREEN.to_string(), line_of(&src, "when showingDetail"))]
    );
    let (_, warnings) = contract::compile_warned(&src);
    let m = &warnings[0].message;
    assert!(
        m.starts_with("this `when` swaps the page between screens by state")
            && m.contains("the browser's Back leaves the app, a reload loses the place and no screen can be linked")
            && m.contains("`routes` with a path for each, `push` to open one")
            && m.contains("`contract vocab screen`"),
        "{m}"
    );
    // The same screens through a `match` on the chosen option, and as a
    // chain with no last `else`, after a loading arm (flashcards).
    let matched = finder(&format!(
        "match first(chosen)\n  case some(m)\n{}  case none\n{}",
        DETAIL
            .replace("\n  ", "\n    ")
            .replacen("  column", "    column", 1),
        LIST.replace("\n  ", "\n    ")
            .replacen("  column", "    column", 1)
    ));
    assert_eq!(
        warned(&matched),
        [(SCREEN.to_string(), line_of(&matched, "match first"))]
    );
    let chain = finder(&format!("when pending(meals)\n  progress aria-label=\"Loading\"\nelse when length(chosen) == 0\n{LIST}else when length(chosen) > 0\n  each m in chosen key=m.id\n{}", DETAIL.replace("\n  ", "\n    ").replacen("  column", "    column", 1)));
    assert_eq!(
        warned(&chain),
        [(SCREEN.to_string(), line_of(&chain, "when pending"))]
    );
    // event-rsvp's: a create form and an event's detail in one pane, the
    // form's Cancel the press that leaves it.
    let rsvp = "shape Event\n  id: string\n  title: string\ncomponent App\n  state selected = \"1\"\n  state creating = false\n  state title = \"\"\n  state answer = \"\"\n  resource events = loadEvents() as shape list<Event> else empty()\n  action newEvent\n    creating = true\n  action back\n    selected = \"\"\n    creating = false\n  action editTitle(v: string)\n    title = v\n  action create\n    creating = false\n  action rsvp(r: string)\n    answer = r\n  view\n    main height=\"100%\" display=\"flex\" flex-direction=\"column\"\n      row padding=16\n        text \"gather\"\n        button press=newEvent testId=\"new-event\"\n          text \"Create event\"\n      scroll flex=1 min-height=0\n        column flex=1 min-width=0 gap=18 testId=\"detail-pane\"\n          when creating\n            column gap=22\n              column gap=8\n                text \"A new reason to get together.\" role=\"heading\" aria-level=2\n              input value=title input=editTitle aria-label=\"Event title\"\n              row gap=10\n                button press=create testId=\"create-save\"\n                  text \"Create event\"\n                button press=back testId=\"create-cancel\"\n                  text \"Cancel\"\n          else\n            each e in filter(events, x => x.id == selected) key=e.id\n              column gap=24\n                column gap=10\n                  text e.title role=\"heading\" aria-level=2\n                row role=\"radiogroup\" aria-label=\"Your RSVP\"\n                  button role=\"radio\" aria-checked=(answer == \"going\") press=rsvp(\"going\")\n                    text \"Going\"\n                  button role=\"radio\" aria-checked=(answer == \"maybe\") press=rsvp(\"maybe\")\n                    text \"Maybe\"\n";
    assert_eq!(
        warned(rsvp),
        [(SCREEN.to_string(), line_of(rsvp, "when creating"))]
    );
}

#[test]
fn routes_tabs_loading_empty_disclosure_and_overlays_are_not_named() {
    let detail = DETAIL
        .replace("\n  ", "\n    ")
        .replacen("  column", "    column", 1);
    let swap = format!("when showingDetail\n  each m in chosen key=m.id\n{detail}else\n{LIST}");
    // A router's screens are its routes.
    let routed = finder(&swap).replacen("shape Meal", "routes nav\n  home \"/\"\nshape Meal", 1);
    assert_eq!(warned(&routed), []);
    for (case, choice) in [
        // Tabs: a state a `tab` reads its selection from.
        ("tabs", format!("row role=\"tablist\"\n  button role=\"tab\" aria-selected=(tab == \"all\") press=pick(\"all\")\n    text \"All\"\n  button role=\"tab\" aria-selected=(tab != \"all\") press=pick(\"saved\")\n    text \"Saved\"\nwhen tab == \"all\"\n{LIST}else\n{}", LIST.replace("What sounds good today?", "Saved"))),
        // A loading or error view: keyed on a resource.
        ("loading", format!("when pending(meals)\n  each m in meals key=m.id\n{detail}else\n{LIST}")),
        ("responsive", format!("when viewport.name == \"wide\"\n{LIST}else\n{LIST}")),
        // An empty state: one control.
        ("empty", format!("when length(chosen) == 0\n  column\n    text \"Nothing chosen yet\" role=\"heading\"\n    button press=edit(\"\")\n      text \"Search\"\nelse\n{LIST}")),
        // Disclosure: an arm with no heading.
        ("disclosure", format!("when open2\n{LIST}else\n  button press=toggle\n    text \"Show the recipes\"\n  button press=edit(\"\")\n    text \"Clear\"")),
        // A swap no press inside it moves (the bar outside does).
        ("unmoved", format!("when showingDetail\n  each m in chosen key=m.id\n{}else\n{}", detail.replace("press=back", "press=share(m)"), LIST.replace("press=open(m)", "press=share(m)"))),
        // Inside a row, a dialog or a popover: theirs.
        ("row", format!("each n in meals key=n.id\n  {}", swap.replace('\n', "\n  "))),
        ("dialog", format!("dialog id=\"d\" closedby=\"any\"\n  {}", swap.replace('\n', "\n  "))),
        ("popover", format!("column id=\"p\" popover=\"auto\"\n  {}", swap.replace('\n', "\n  "))),
    ] {
        let src = finder(&choice);
        assert_eq!(warned(&src), [], "{case}\n{src}");
    }
    // A kept setting: a `persist` state.
    let kept = finder(
        &format!("when tab == \"all\"\n  each m in chosen key=m.id\n{detail}else\n{LIST}")
            .replace("press=back", "press=pick(\"x\")"),
    )
    .replace("state tab = \"all\"", "state tab = \"all\" persist");
    assert_eq!(warned(&kept), []);
}

const WIDTH_PADDING: &str = "lower-width-padding";

#[test]
fn a_full_width_box_with_padding_is_named_with_its_repair() {
    // LLP 1116.009 D5: focus-timer's and recipe-finder's round-11 bodies,
    // `column padding=24 width="100%"`, which ran past a phone's right
    // side while `verify` said nothing. The parent is the view's column
    // (lines 7 on).
    let src = buttons(
        "column padding=24 gap=12 width=\"100%\"\n  text \"Timer\"\nbox width=\"100%\" padding=\"8px 16px\"\n  text \"x\"\nview width=\"100%\" border=\"1px solid -exact-separator\"\n  text \"y\"\nscroll flex=1\n  column padding-left=16 width=\"100%\"\n    text \"z\"",
    );
    assert_eq!(
        warned(&src),
        [
            (WIDTH_PADDING.to_string(), 7),
            (WIDTH_PADDING.to_string(), 9),
            (WIDTH_PADDING.to_string(), 11),
            (WIDTH_PADDING.to_string(), 14)
        ]
    );
    let (_, warnings) = contract::compile_warned(&src);
    let m = &warnings[0].message;
    assert!(
        m.starts_with("`width=\"100%\"` with padding is wider than its parent by the padding")
            && m.contains("drop the width (a block or a stretched flex item fills its parent")
            && m.contains("`flex=1` field `min-width=0`")
            && m.contains("`box-sizing=\"border-box\"`"),
        "{m}"
    );
    // A style's rows count as the node's, its own winning.
    let styled = "style Panel\n  padding=16 width=\"100%\"\n\ncomponent App\n  view\n    column\n      column class=Panel\n        text \"a\"\n      column class=Panel box-sizing=\"border-box\"\n        text \"b\"\n      column class=Panel padding=0\n        text \"c\"\n";
    assert_eq!(warned(styled), [(WIDTH_PADDING.to_string(), 7)]);
}

#[test]
fn what_fits_or_the_compiler_cannot_see_is_not_named_for_width_and_padding() {
    for (case, view) in [
        // The author's own word on sizing, or no room taken.
        (
            "border-box",
            "column padding=24 width=\"100%\" box-sizing=\"border-box\"\n  text \"a\"",
        ),
        (
            "a width that leaves room",
            "column padding=8 width=\"50%\"\n  text \"a\"",
        ),
        ("a fixed width", "column padding=24 width=320\n  text \"a\""),
        ("no padding", "column width=\"100%\"\n  text \"a\""),
        (
            "vertical padding only",
            "column padding=\"16px 0\" width=\"100%\"\n  text \"a\"",
        ),
        (
            "padding taken back",
            "column padding=24 padding-left=0 padding-right=0 width=\"100%\"\n  text \"a\"",
        ),
        (
            "a border with no line",
            "column border=\"2px none -exact-separator\" width=\"100%\"\n  text \"a\"",
        ),
        // A value the compiler cannot read.
        (
            "a computed padding",
            "column padding=(label == \"\" ? 0 : 24) width=\"100%\"\n  text \"a\"",
        ),
        (
            "a computed width",
            "column padding=24 width=(label == \"\" ? \"100%\" : \"50%\")\n  text \"a\"",
        ),
        (
            "a length it does not read",
            "column padding=\"env(safe-area-inset-left)\" width=\"100%\"\n  text \"a\"",
        ),
        // Not in the parent's flow, or not a container.
        (
            "positioned",
            "column padding=24 width=\"100%\" position=\"absolute\"\n  text \"a\"",
        ),
        (
            "hidden",
            "column padding=24 width=\"100%\" display=\"none\"\n  text \"a\"",
        ),
        ("a field", "input width=\"100%\" padding=12 value=label"),
        // A row's items shrink to fit; a grid's resolve against a cell.
        (
            "in a row",
            "row\n  column padding=24 width=\"100%\"\n    text \"a\"",
        ),
        (
            "in a grid",
            "view display=\"grid\"\n  column padding=24 width=\"100%\"\n    text \"a\"",
        ),
        (
            "in a flex row",
            "view display=\"flex\"\n  column padding=24 width=\"100%\"\n    text \"a\"",
        ),
        (
            "a slot's children",
            "Pill()\n  column padding=24 width=\"100%\"\n    text \"a\"",
        ),
    ] {
        let src = buttons(view);
        assert_eq!(warned(&src), [], "{case}\n{src}");
    }
    // A flex column and an explicit block both stretch their items.
    for parent in [
        "view display=\"flex\" flex-direction=\"column\"",
        "view display=\"block\"",
    ] {
        let src = buttons(&format!(
            "{parent}\n  column padding=24 width=\"100%\"\n    text \"a\""
        ));
        assert_eq!(warned(&src), [(WIDTH_PADDING.to_string(), 8)], "{src}");
    }
}

#[test]
fn a_grouped_lists_rows_are_not_named_for_width_and_padding() {
    // The grouped list's sections place their rows themselves (lowering's,
    // not the author's block): not a parent a percentage is of.
    let src = buttons(
        "list appearance=\"auto\" listStyle=\"inset-grouped\" flex=1 min-height=0\n  section\n    row width=\"100%\" padding-left=16\n      text \"a\"",
    );
    assert_eq!(warned(&src), []);
}

const ROUND_BUTTON: &str = "lower-round-button-label";

/// The warnings of an app that uses `exact:content`'s `RoundButton`, as a build prints them:
/// `body` is the view's lines, indented, from line 8.
fn warned_circles(name: &str, body: &str) -> Vec<(String, u32, String)> {
    let src = format!(
        "use RoundButton from \"exact:content\"\ncomponent App\n  state n = 0\n  state title = \"Start quiz\"\n  action go\n    n = n + 1\n  view\n    column\n{body}"
    );
    let dir = std::env::temp_dir().join(format!("exact-circles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, &src).unwrap();
    let (result, warnings) = contract::compile_path_all_warned(&path, false);
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(e) = result {
        panic!("a warning never fails a build: {e:?}\n{src}");
    }
    warnings
        .into_iter()
        .map(|w| (w.id.clone(), w.span.line, w.message))
        .collect()
}

#[test]
fn a_round_button_label_longer_than_the_circle_holds_is_named_with_its_repair() {
    // trivia, round 13: `RoundButton(label="Start quiz")` drew "Start q…" in the 80-point
    // circle; the warning points at the label and names the prominent button.
    let call = |label: &str| {
        format!(
            "      RoundButton(label={label}, tint=none, press=go, disabled=false, testId=\"start\")\n"
        )
    };
    let got = warned_circles("long", &call("\"Start quiz\""));
    assert_eq!(
        got.iter()
            .map(|(id, line, _)| (id.as_str(), *line))
            .collect::<Vec<_>>(),
        [(ROUND_BUTTON, 9)],
        "{got:?}"
    );
    assert!(
        got[0].2.contains("the Clock's circle for a word")
            && got[0]
                .2
                .contains("\"Start quiz\" is 10 characters and the 80-point circle holds about 7")
            && got[0].2.contains("`button` with `buttonStyle=\"filled\"`"),
        "{got:?}"
    );
    // One word too long for the circle: eight characters is past the seven it holds.
    let got = warned_circles("word", &call("\"Continue\""));
    assert!(
        got.len() == 1
            && got[0]
                .2
                .contains("\"Continue\" is 8 characters and the 80-point circle holds about 7"),
        "{got:?}"
    );
    // The Clock's words, the one-screen pattern's "500 ml" (two words, six characters) and a
    // label the app computes are the circle's to hold.
    for label in [
        "\"Start\"",
        "\"Stop\"",
        "\"Lap\"",
        "\"Reset\"",
        "\"Resume\"",
        "\"Restart\"",
        "title",
        "`Go ${n}`",
    ] {
        assert_eq!(warned_circles("fine", &call(label)), [], "{label}");
    }
    // Every written call is named once, in a choice or a loop, at its own line.
    let body = format!(
        "{}      each t in [\"a\", \"b\"] key=t\n        RoundButton(label=\"Next question\", tint=none, press=go, disabled=false, testId=t)\n",
        call("\"Play again\"")
    );
    let got = warned_circles("sites", &body);
    assert_eq!(
        got.iter()
            .map(|(id, line, _)| (id.as_str(), *line))
            .collect::<Vec<_>>(),
        [(ROUND_BUTTON, 9), (ROUND_BUTTON, 11)],
        "{got:?}"
    );
}

#[test]
fn an_authors_own_round_button_is_not_the_circle() {
    // A component of the name that takes other props is not `exact:content`'s part.
    let src = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      RoundButton(label=\"Start the quiz now\", press=go)\ncomponent RoundButton\n  props\n    label: string\n    press: action\n  view\n    button press=press\n      text label\n";
    assert_eq!(warned(src), []);
}
