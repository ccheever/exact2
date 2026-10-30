//! LLP 1116.010 D5: a miswiring the compiler can see is a warning. A name
//! that points at nothing, or a key that never meets its partner, which a
//! host would refuse or ignore at run time; each names its repair, fails no
//! build, and what the compiler cannot see through is given the benefit.

/// The ids and lines of the warnings `src` earns, after checking that it
/// still compiles.
fn warned(src: &str) -> Vec<(String, u32)> {
    let (result, warnings) = contract::compile_warned(src);
    if let Err(e) = result {
        panic!("a warning never fails a build: {e}\n{src}");
    }
    warnings
        .iter()
        .map(|w| (w.id.clone(), w.span.line))
        .collect()
}

fn messages(src: &str) -> Vec<String> {
    contract::compile_warned(src)
        .1
        .into_iter()
        .map(|w| w.message)
        .collect()
}

const SWIPE: &str = "lower-swipe-wiring";

/// A list of rows, each a swipe `scroll`; `row` is what is in the scroll, `after` is
/// written beside it, in the row's `for` (lines 9 on).
fn rows(attrs: &str, row: &str, after: &str) -> String {
    let indent = |text: &str, by: usize| -> String {
        text.lines()
            .map(|l| format!("{}{l}\n", " ".repeat(by)))
            .collect()
    };
    format!(
        "component App\n  state items = [\"Milk\", \"Eggs\"]\n  action remove(item: string)\n    items = items.filter(i => i != item)\n  view\n    column\n      for i in items key=i\n        scroll {attrs} overflow-x=\"scroll\" overflow-y=\"hidden\" scroll-snap-type=\"x mandatory\"\n{}{}",
        indent(row, 10),
        indent(after, 8)
    )
}

const ROW: &str =
    "row id=`row-${i}` width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\"\n  text i";
const DELETE: &str = "button id=`delete-${i}` destructive=true press=remove(i) aria-label=`Delete ${i}`\n  image \"symbol:delete\"";

#[test]
fn a_swipe_action_outside_its_scroll_is_named_with_its_repair() {
    // grocery-list's seven minutes: the delete button indented as the scroll's
    // sibling, not its child. iOS refused the row; the web drew the button
    // as a second line under each item.
    let src = rows(
        "swipeContent=`row-${i}` swipeTrailing=`delete-${i}`",
        ROW,
        DELETE,
    );
    assert_eq!(warned(&src), [(SWIPE.to_string(), 8)]);
    let m = &messages(&src)[0];
    assert!(
        m.contains("`swipeTrailing` names `delete-${i}`, but no element inside this `scroll` has that `id`")
            && m.contains("Put the button inside this `scroll`")
            && m.contains("docs/recipes/list-rows.md"),
        "{m}"
    );
    // A literal list, one name wrong, and the content's own name.
    let src = rows(
        "swipeContent=\"body\" swipeLeading=\"pin delete\" swipeTrailing=\"archive\"",
        "row id=\"body\" width=\"100%\"\n  text i\nbutton id=\"pin\" press=remove(i) aria-label=\"Pin\"\n  image \"symbol:pin\"\nbutton id=\"archive\" press=remove(i) aria-label=\"Archive\"\n  image \"symbol:filter\"",
        "",
    );
    let m = messages(&src);
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains("`swipeLeading` names \"delete\""), "{m:?}");
    let src = rows(
        "swipeContent=\"row\" swipeTrailing=\"delete\"",
        "row id=\"body\"\n  text i\nbutton id=\"delete\" press=remove(i) aria-label=\"x\"\n  image \"symbol:delete\"",
        "",
    );
    assert!(
        messages(&src)[0].contains("`swipeContent` names \"row\", but no element inside"),
        "{:?}",
        messages(&src)
    );
}

#[test]
fn a_swipe_row_wired_inside_its_scroll_is_not_named() {
    // The recipe's row, with its action inside; a template and a literal
    // that could meet are one; and a computed id is the benefit.
    let inside = rows(
        "swipeContent=`row-${i}` swipeTrailing=`delete-${i}`",
        &format!("{ROW}\n{DELETE}"),
        "",
    );
    assert_eq!(warned(&inside), []);
    let literal = rows(
        "swipeContent=\"row\" swipeTrailing=\"delete\"",
        "row id=\"row\" width=\"100%\"\n  text i\nbutton id=\"delete\" press=remove(i) aria-label=\"x\"\n  image \"symbol:delete\"",
        "",
    );
    assert_eq!(warned(&literal), []);
    let computed = rows(
        "swipeContent=`row-${i}` swipeTrailing=`delete-${i}`",
        &format!("{ROW}\nbutton id=(i == \"Milk\" ? \"a\" : \"b\") press=remove(i) aria-label=\"x\"\n  image \"symbol:delete\""),
        "",
    );
    assert_eq!(warned(&computed), []);
    let mixed = rows(
        "swipeContent=\"row-Milk\" swipeTrailing=`delete-${i}`",
        &format!("{ROW}\n{DELETE}"),
        "",
    );
    assert_eq!(warned(&mixed), []);
    // A choice of actions, euxy's Reset on presets alone: each arm's names are
    // inside, and an empty arm is a row with no action that time.
    for trailing in [
        "(i == \"Milk\" ? `reset-${i} delete-${i}` : `delete-${i}`)",
        "(i == \"Milk\" ? `delete-${i}` : \"\")",
    ] {
        let chosen = rows(
            &format!("swipeContent=`row-${{i}}` swipeTrailing={trailing}"),
            &format!("{ROW}\nbutton id=`reset-${{i}}` press=remove(i) aria-label=\"Reset\"\n  image \"symbol:delete\"\n{DELETE}"),
            "",
        );
        assert_eq!(warned(&chosen), [], "{trailing}");
    }
}

#[test]
fn a_choice_of_swipe_actions_is_read_arm_by_arm() {
    let row = |trailing: &str| {
        rows(
            &format!("swipeContent=`row-${{i}}` swipeTrailing={trailing}"),
            &format!("{ROW}\n{DELETE}"),
            "",
        )
    };
    // An arm that names a button outside the scroll is refused when it is taken.
    let src = row("(i == \"Milk\" ? `reset-${i} delete-${i}` : `delete-${i}`)");
    assert_eq!(warned(&src), [(SWIPE.to_string(), 8)]);
    assert!(
        messages(&src)[0].contains("`swipeTrailing` names `reset-${i}`"),
        "{:?}",
        messages(&src)
    );
    // Arms that name nothing: no action on any row.
    let src = row("(i == \"Milk\" ? \"\" : \"\")");
    assert!(
        messages(&src)[0].contains("written without `swipeLeading` or `swipeTrailing`"),
        "{:?}",
        messages(&src)
    );
    // An action the compiler cannot read is the benefit of the doubt, and an
    // arm it cannot read hides nothing the other arm names.
    assert_eq!(warned(&row("i")), []);
    let src = row("(i == \"Milk\" ? \"orphan\" : i)");
    assert_eq!(warned(&src), [(SWIPE.to_string(), 8)]);
    assert!(
        messages(&src)[0].contains("`swipeTrailing` names \"orphan\""),
        "{:?}",
        messages(&src)
    );
    assert_eq!(warned(&row("(i == \"Milk\" ? \"\" : i)")), []);
}

#[test]
fn a_swipe_action_inside_the_content_or_without_the_other_is_named() {
    // The action in the content box: iOS refuses `… is inside swipeContent`.
    let src = rows(
        "swipeContent=`row-${i}` swipeTrailing=`delete-${i}`",
        "row id=`row-${i}` width=\"100%\"\n  text i\n  button id=`delete-${i}` press=remove(i) aria-label=\"x\"\n    image \"symbol:delete\"",
        "",
    );
    assert_eq!(warned(&src), [(SWIPE.to_string(), 8)]);
    assert!(
        messages(&src)[0].contains("sits inside the `swipeContent` element"),
        "{:?}",
        messages(&src)
    );
    // A content with no action, an action with no content: refused or ignored.
    let none = rows("swipeContent=`row-${i}`", ROW, "");
    assert!(
        messages(&none)[0].contains("written without `swipeLeading` or `swipeTrailing`"),
        "{:?}",
        messages(&none)
    );
    let orphan = rows(
        "swipeTrailing=`delete-${i}`",
        &format!("{ROW}\n{DELETE}"),
        "",
    );
    assert!(
        messages(&orphan)[0].contains("`swipeTrailing` is written without a `swipeContent`"),
        "{:?}",
        messages(&orphan)
    );
}

const KEY: &str = "lower-navigation-key";

/// A stack whose root is keyed `root`, its list route `home` and its pushed route `pushed`
/// (lines 8 and 11).
fn stack(root: &str, home: &str, pushed: &str) -> String {
    format!(
        "routes nav\n  rooms \"/\"\n    room \"/room/:id\"\ncomponent App\n  view\n    main navigationKey={root} navigationBack=\"back\" width=\"100%\" height=\"100%\"\n      for e in stack(nav) key=e.id\n        if e.name == \"rooms\"\n          column navigationKey={home} position=\"absolute\" inset=0\n            text \"Rooms\"\n        if e.name == \"room\"\n          column navigationKey={pushed} position=\"absolute\" inset=0\n            text \"Room\"\n"
    )
}

#[test]
fn a_navigation_key_that_never_meets_its_route_is_named_with_its_repair() {
    // chat-rooms, round 12: `main navigationKey="rooms"` over routes keyed by id,
    // so a tap on a room pushed nothing that was ever shown (eight minutes).
    let src = stack("\"rooms\"", "\"rooms\"", "`${e.id}`");
    assert_eq!(warned(&src), [(KEY.to_string(), 6)]);
    let m = &messages(&src)[0];
    assert!(
        m.contains("the navigation root's `navigationKey` is the literal \"rooms\"")
            && m.contains("its routes are keyed by their entry's id (`${e.id}`)")
            && m.contains("navigationKey=`${top(nav).id}`"),
        "{m}"
    );
    // The same app's final wiring: a root of the visit's id over a route keyed by a
    // word, which the root never names ("0" at the stack's root; agent-pitfalls).
    let src = stack("`${top(nav).id}`", "\"rooms\"", "`${e.id}`");
    assert_eq!(warned(&src), [(KEY.to_string(), 9)]);
    let m = &messages(&src)[0];
    assert!(
        m.contains("this route's `navigationKey` is the literal \"rooms\"")
            && m.contains("`navigationKey \"0\" matches no route`")
            && m.contains("Key every route `navigationKey=`${e.id}``"),
        "{m}"
    );
}

#[test]
fn navigation_keys_that_meet_or_that_the_compiler_cannot_read_are_not_named() {
    for (case, root, home, pushed) in [
        // The patterns': every route by its entry, under the visit on top.
        ("by id", "`${top(nav).id}`", "`${e.id}`", "`${e.id}`"),
        // The `rest` pattern's: a choice names the list's own word while it is on top.
        (
            "a choice",
            "(top(nav).name == \"rooms\" ? \"rooms\" : `${top(nav).id}`)",
            "\"rooms\"",
            "`${e.id}`",
        ),
        // Words all through, or a key the compiler cannot read.
        ("words", "\"rooms\"", "\"rooms\"", "\"room\""),
        ("a computed root", "(e0)", "`${e.id}`", "`${e.id}`"),
        (
            "the stack's own first key",
            "`${top(nav).id}`",
            "\"0\"",
            "`${e.id}`",
        ),
    ] {
        let src = stack(root, home, pushed).replace("(e0)", "`${top(nav).name}`");
        assert_eq!(warned(&src), [], "{case}\n{src}");
    }
}

const SPLIT: &str = "lower-navigation-split";

#[test]
fn a_navigation_split_where_no_host_reads_it_is_named_with_its_repair() {
    // recipe-finder, round 12 (and seven apps of rounds 10 and 11): `navigationSplit`
    // on `main`, which the split reads on the root route only.
    let on_root = stack("`${top(nav).id}`", "`${e.id}`", "`${e.id}`").replace(
        "navigationBack=\"back\"",
        "navigationBack=\"back\" navigationSplit=360",
    );
    assert_eq!(warned(&on_root), [(SPLIT.to_string(), 6)]);
    let m = &messages(&on_root)[0];
    assert!(
        m.contains("`navigationSplit` is on the navigation root (this `main`)")
            && m.contains("it belongs to the stack's root route"),
        "{m}"
    );
    let on_box = stack("`${top(nav).id}`", "`${e.id}`", "`${e.id}`").replace(
        "text \"Rooms\"",
        "box navigationSplit=\"none\"\n              text \"Rooms\"",
    );
    assert_eq!(warned(&on_box), [(SPLIT.to_string(), 10)]);
    // On the root route, and a placeholder box inside it, it is read.
    let on_route = stack("`${top(nav).id}`", "`${e.id}`", "`${e.id}`").replace(
        "column navigationKey=`${e.id}` position=\"absolute\" inset=0\n            text \"Rooms\"",
        "column navigationKey=`${e.id}` navigationSplit=\"none\" position=\"absolute\" inset=0\n            box navigationSplit=\"placeholder\"\n              text \"Pick one\"",
    );
    assert_eq!(warned(&on_route), []);
}

const IMAGE: &str = "lower-image-source";

/// The warnings of a file that uses a package part (`exact:content`), as a build prints them.
fn warned_file(name: &str, src: &str) -> Vec<(String, u32, String)> {
    let dir = std::env::temp_dir().join(format!("exact-miswiring-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, src).unwrap();
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
fn an_image_source_that_is_neither_a_symbol_nor_a_file_is_named_with_its_repair() {
    // habit-tracker, round 12: `EmptyState(symbol="sparkles")` drew a blank, and event-rsvp's
    // `symbol="calendar"` the same; the warning points at the call's own line.
    let src = "use EmptyState from \"exact:content\"\ncomponent App\n  view\n    column\n      EmptyState(symbol=\"sparkles\", title=\"Nothing\", message=\"Add one\")\n      image \"gear\"\n      image \"banner\"\n";
    let got = warned_file("blank", src);
    assert_eq!(
        got.iter()
            .map(|(id, line, _)| (id.as_str(), *line))
            .collect::<Vec<_>>(),
        [(IMAGE, 5), (IMAGE, 6), (IMAGE, 7)],
        "{got:?}"
    );
    assert!(
        got[0]
            .2
            .contains("this word is a symbol: write `symbol:sparkles`")
            && got[1].2.contains("write `symbol:settings`")
            && got[2].2.contains("a symbol is written `symbol:<role>`"),
        "{got:?}"
    );
    // A role, an Apple name, a file, a URL, a computed source: all drawn.
    let fine = "component App\n  state s = \"x\"\n  view\n    column\n      image \"symbol:heart\"\n      image \"symbol:sf/heart.text.square\"\n      image \"logo.png\"\n      image \"/img/logo\"\n      image \"https://example.com/a\"\n      image s\n";
    assert_eq!(warned(fine), []);
}

const ROOT: &str = "lower-replace-root";

/// A welcome at `/` beside a main list at `/habits`, as Codex Sol wrote it: the
/// action `begin` is `body` (line 10 on), pressed on the route that `cond` chooses;
/// `follow` is the navigation root's `navigate`, as in every pattern.
fn first_screen(body: &str, cond: &str) -> String {
    format!(
        "routes nav\n  welcome \"/\"\n  habits \"/habits\"\n    habit \"/habit/:id\"\ncomponent App\n  state welcomed = false\n  action follow(url: string)\n    nav = go(nav, url)\n  action begin\n{body}  view\n    main navigationKey=`${{top(nav).id}}` navigationBack=\"back\" navigate=follow width=\"100%\" height=\"100%\"\n      for e in stack(nav) key=e.id\n        if {cond}\n          column navigationKey=`${{e.id}}` position=\"absolute\" inset=0\n            button press=begin testId=\"continue\"\n              text \"Continue\"\n"
    )
}

const WELCOME: &str = "e.name == \"welcome\"";
const LEAVE: &str = "    nav = replace(nav, path(\"habits\"))\n";
/// An action that pushes a second visit of the welcome, before `begin`.
const AHEAD: &str = "  action ahead\n    nav = push(nav, \"/?again=1\")\n  action begin\n";
/// The shapes a sequence's `play` takes.
const TRACKS: &str = "shape Step\n  beat: number\n  src: string\n  gain: number\n  group: string\n  note: number\n  velocity: number\n  channel: number\n  duration: number\n  haptic: string\nshape Track\n  beats: number\n  loops: number\n  offset: number\n  output: string\n  steps: Step[]\n";
const BUTTON: &str =
    "            button press=begin testId=\"continue\"\n              text \"Continue\"\n";

/// The fixture with `extra` in the welcome route after Continue.
fn beside(extra: &str) -> String {
    first_screen(LEAVE, WELCOME).replace(BUTTON, &format!("{BUTTON}{extra}"))
}

#[test]
fn a_replace_the_router_refuses_at_launch_is_named_with_its_repair() {
    // The welcome app (guard-rail evaluation, 2026-10-16): Continue replaced the
    // root with the main list, which the router refuses while the root is alone.
    let src = first_screen(&format!("    welcomed = true\n{LEAVE}"), WELCOME);
    assert_eq!(warned(&src), [(ROOT.to_string(), 11)]);
    let m = &messages(&src)[0];
    assert!(
        m.contains("if the app boots and draws its launch screen, this `replace`, pressed there before anything else changes the router, is refused and `habits` does not open")
            && m.contains("a launch at `/` opens `welcome` as the only screen of its stack")
            && m.contains("the log says \"replace cannot change the tab's root route\"")
            && m.contains("show what `welcome` shows inside it (docs/recipes/first-launch.md)")
            && m.contains("write `push`"),
        "{m}"
    );
    // A literal location with a query, a `path()` of literals beside a command, the
    // top named in the view, an action that calls the one that replaces, and what a
    // person may do before Continue, which the claim leaves out: another button, a
    // link, the navigate handler pressed with a location.
    let again = |extra: &str| {
        beside(extra).replace(
            "  action begin\n",
            "  action again\n    nav = push(nav, \"/?again=1\")\n  action begin\n",
        )
    };
    for (case, src) in [
        ("a literal", first_screen("    nav = replace(nav, \"/habits?from=welcome\")\n", WELCOME)),
        ("a path", first_screen("    haptic(\"success\")\n    nav = replace(nav, path(\"habit\", \"7\"))\n", WELCOME)),
        ("the top", first_screen(LEAVE, "top(nav).name == \"welcome\"")),
        ("a call", first_screen("    leave()\n  action leave\n    nav = replace(nav, path(\"habits\"))\n", WELCOME)),
        ("another button", again("            button press=again\n              text \"Again\"\n")),
        ("a link", beside("            link href=\"/?again=1\"\n              text \"Again\"\n")),
        ("the navigate handler pressed", beside("            button press=follow(\"/?again=1\")\n              text \"Again\"\n")),
        // A list, which a wide window shows beside a pushed screen: none is
        // pushed before the press.
        ("a split", beside("            list\n              text \"Rows\"\n")),
        ("a task that is refused", first_screen(LEAVE, WELCOME).replace("  action begin\n", "  task t mount\n    after(1, resume)\n  action resume\n    if welcomed\n      nav = replace(nav, path(\"habits\"))\n  action begin\n")),
    ] {
        let named = warned(&src).into_iter().filter(|(id, _)| id == ROOT).count();
        assert_eq!(named, 1, "{case}\n{src}");
    }
}

#[test]
fn a_replace_that_may_not_be_refused_or_reached_at_launch_is_not_named() {
    for (case, src) in [
        // Pressed on a pushed screen, where it replaces that screen.
        ("a pushed screen", first_screen(LEAVE, "e.name == \"habit\"")),
        ("no choice", first_screen(LEAVE, "e.id >= 0")),
        ("an and", first_screen(LEAVE, "e.name == \"welcome\" && e.id >= 0")),
        // Something that moves the router by itself first: a task, or another
        // handler in the same gesture.
        ("a task", first_screen(LEAVE, WELCOME).replace("  action begin\n", "  task t mount\n    after(1, ahead)\n  action ahead\n    nav = open(nav, path(\"habits\"))\n  action begin\n")),
        // An automatic handler whose action binds the router's name to another router.
        ("an automatic router parameter", beside("").replace("      for e in stack(nav)", "      column intersect=ahead(push(push(nav, \"/?one=1\"), \"/?two=1\")) width=1 height=1\n      for e in stack(nav)").replace("  action begin\n", "  action ahead(nav: Router)\n    nav = back(nav)\n  action begin\n")),
        ("an automatic match binding", beside("").replace("      for e in stack(nav)", "      column intersect=ahead(some(push(push(nav, \"/?one=1\"), \"/?two=1\"))) width=1 height=1\n      for e in stack(nav)").replace("  action begin\n", "  action ahead(r: option<Router>)\n    match r\n      case some(nav)\n        nav = back(nav)\n      case none\n  action begin\n")),
        // A link around the button, which the click follows first.
        ("a link root", first_screen(LEAVE, WELCOME).replace("    main navigationKey", "    link href=\"/?again=1\" navigationKey")),
        ("a link inside", first_screen(LEAVE, WELCOME).replace("              text \"Continue\"\n", "              link href=\"/?again=1\"\n                text \"Continue\"\n")),
        ("a pressed root", first_screen(LEAVE, WELCOME).replace("    main navigationKey", "    main press=ahead navigationKey").replace("  action begin\n", "  action ahead\n    nav = open(nav, path(\"habits\"))\n  action begin\n")),
        ("a link around", first_screen(LEAVE, WELCOME).replace(BUTTON, "            link href=\"/?again=1\"\n              button press=begin\n                text \"Continue\"\n")),
        ("a native view's press", beside("").replace("      for e in stack(nav)", "      startup-view press=ahead width=1 height=1\n      for e in stack(nav)").replace("  action begin\n", "  action ahead\n    nav = push(nav, \"/?again=1\")\n  action begin\n")),
        ("a mutation's then", first_screen(LEAVE, WELCOME).replace("component App\n", "shape Reply\n  ok: bool\ncomponent App\n  mutation saved as shape Reply then ahead\n").replace("  action begin\n", AHEAD)),
        ("a sequence's ended", first_screen(LEAVE, WELCOME).replace("component App\n", &format!("{TRACKS}component App\n")).replace("  action begin\n", &format!("  sequence s when welcomed\n    play([Track(beats=1, loops=1, offset=0, output=\"synth\", steps=[])], bpm=120, ended=ahead)\n{AHEAD}"))),
        ("a pointerdown", first_screen(LEAVE, WELCOME).replace("button press=begin", "button pointerdown=ahead press=begin").replace("  action begin\n", "  action ahead\n    nav = open(nav, path(\"habits\"))\n  action begin\n")),
        // A view guard, a disabled, hidden or styled control or root, a press that
        // is not a button's, and a root keyed otherwise.
        ("a guard", first_screen(LEAVE, WELCOME).replace(BUTTON, "            if depth(nav) > 1\n              button press=begin\n                text \"Continue\"\n")),
        ("a disabled control", first_screen(LEAVE, WELCOME).replace("press=begin", "press=begin disabled=(welcomed)")),
        ("a class", format!("style Off\n  display=\"none\"\n{}", first_screen(LEAVE, WELCOME)).replace("button press=begin", "button class=Off press=begin")),
        ("a class on the root", format!("style Off\n  display=\"none\"\n{}", first_screen(LEAVE, WELCOME)).replace("main navigationKey", "main class=Off navigationKey")),
        ("an opacity", first_screen(LEAVE, WELCOME).replace("button press=begin", "button opacity=0 press=begin")),
        ("a hidden input", first_screen(LEAVE, WELCOME).replace(BUTTON, "            input type=\"hidden\" press=begin\n")),
        ("a pinned root", first_screen(LEAVE, WELCOME).replace("main navigationKey=`${top(nav).id}`", "main navigationKey=(welcomed ? \"0\" : `${top(nav).id}`)")),
        ("a route keyed by a word", first_screen(LEAVE, WELCOME).replace("column navigationKey=`${e.id}`", "column navigationKey=\"welcome\"")),
        ("an ancestor's press", first_screen(LEAVE, WELCOME).replace(BUTTON, "            column press=ahead\n              button press=begin\n                text \"Continue\"\n").replace("  action begin\n", "  action ahead\n    nav = open(nav, path(\"habits\"))\n  action begin\n")),
        // An argument evaluated before the action, which may refuse.
        ("a bound argument", first_screen(LEAVE, WELCOME).replace("action begin\n", "action begin(ignored: string)\n").replace("press=begin", "press=begin(path(\"habit\", `${welcomed}`))")),
        // A function of the app's that takes a name the router or `path()` calls:
        // here the replace goes to `/` and succeeds.
        ("a shadowed path", format!("fn path(route: string): string = \"/\"\n{}", first_screen(LEAVE, WELCOME))),
        ("a shadowed segment", format!("fn encodeRouteSegment(value: string): string = \"..?done=1\"\n{}", first_screen("    nav = replace(nav, path(\"habit\", \"7\"))\n", WELCOME))),
        // The navigation stack recipe's swap of a detail, guarded by the top.
        ("a guard on the top", first_screen("    nav = top(nav).name == \"habit\" ? replace(nav, path(\"habit\", \"8\")) : push(nav, path(\"habit\", \"8\"))\n", WELCOME)),
        ("a guard in the action", first_screen(&format!("    if depth(nav) > 1\n  {LEAVE}"), WELCOME)),
        // The root itself, by its route or with a query: the replace succeeds.
        ("the root", first_screen("    nav = replace(nav, path(\"welcome\"))\n", WELCOME)),
        ("the root's query", first_screen("    nav = replace(nav, \"/?filter=done\")\n", WELCOME)),
        // A router a `let` pushed first, a later write, a computed location, a
        // `path()` whose computed parameter may be refused before the replace.
        ("a let", first_screen("    let n = push(nav, path(\"habits\"))\n    nav = replace(n, path(\"habit\", \"7\"))\n", WELCOME)),
        ("a later write", first_screen(&format!("{LEAVE}    nav = push(nav, path(\"habits\"))\n"), WELCOME)),
        ("a computed location", first_screen("    nav = replace(nav, \"/\" + \"habits\")\n", WELCOME)),
        ("a computed parameter", first_screen("    nav = replace(nav, path(\"habit\", \"7\" + \"\"))\n", WELCOME)),
        // A parameter that is a router, not the router's state.
        (
            "a router parameter",
            first_screen(LEAVE, WELCOME)
                .replace("action begin\n", "action begin(nav: Router)\n")
                .replace("press=begin", "press=begin(push(nav, path(\"habits\")))"),
        ),
    ] {
        let named = warned(&src).into_iter().filter(|(id, _)| id == ROOT).count();
        assert_eq!(named, 0, "{case}\n{src}");
    }
    // A launch at `/` that opens no root alone (here `notfound` over the first
    // tab's root), and tabs, whose panels are made as the tablist selects them.
    let src = "routes nav\n  tab welcome \"/a b/one\"\n  section \"/a%20b/:id\"\n  notfound\ncomponent App\n  action begin\n    nav = replace(nav, path(\"section\", \"one\"))\n  view\n    main navigationKey=`${top(nav).id}` navigationBack=\"back\" width=\"100%\" height=\"100%\"\n      for e in stack(nav) key=e.id\n        if e.name == \"welcome\"\n          column navigationKey=`${e.id}` position=\"absolute\" inset=0\n            button press=begin\n              text \"Continue\"\n";
    assert_eq!(warned(src), [], "{src}");
    let src = "routes nav\n  tab today \"/\"\n  tab history \"/history\"\ncomponent App\n  action see\n    nav = replace(nav, path(\"history\"))\n  view\n    main navigationKey=`${top(nav).id}` navigationBack=\"back\"\n      column\n        for t in nav.tabs key=t.name\n          column role=\"tabpanel\" id=`panel-${t.name}`\n            for e in t.stack key=e.id\n              column navigationKey=`${e.id}`\n                if e.name == \"today\"\n                  button press=see\n                    text \"History\"\n      row role=\"tablist\"\n        button role=\"tab\" aria-controls=\"panel-today\" aria-selected=(nav.tab == \"today\")\n          text \"Today\"\n        button role=\"tab\" aria-controls=\"panel-history\" aria-selected=(nav.tab == \"history\")\n          text \"History\"\n";
    assert_eq!(warned(src), [], "{src}");
}

/// The euxy port: a deep link into another tab, `go` then `select`, left the
/// visit on the tab it left; the warning names `open`, which lands it.
#[test]
fn a_select_after_a_push_is_named_with_the_forms_that_land_the_visit() {
    let app = |body: &str| {
        format!("routes nav\n  tab today \"/\"\n  tab patterns \"/patterns\"\n    shared \"/patterns/open\"\ncomponent App\n  action follow(url: string)\n{body}  view\n    main navigationKey=`${{top(nav).id}}` navigate=follow\n      for e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}`\n          text e.name\n")
    };
    const SELECT: &str = "lower-select-after-push";
    let src = app("    let n = go(nav, url)\n    nav = url.startsWith(\"/patterns/\") ? select(n, \"patterns\") : n\n");
    assert_eq!(warned(&src), [(SELECT.to_string(), 8)]);
    let m = &messages(&src)[0];
    assert!(
        m.contains("this `select` does not land the visit on \"patterns\": where `go` pushes, it pushes onto the selected tab's stack, whatever tab declares the route, so the visit stays on the tab being left; where the location is already \"patterns\"'s top, `go` selects \"patterns\" itself and this `select` pops it to its root")
            && m.contains("write `open(nav, url)`, which puts the route's declared chain in place of that tab's stack"),
        "{m}"
    );
    let src = app("    nav = select(push(nav, url), \"patterns\")\n");
    assert_eq!(warned(&src), [(SELECT.to_string(), 7)]);
    // The app's own `push` is no router verb.
    let own = app("    nav = select(push(nav, url), \"patterns\")\n").replace(
        "component App\n",
        "fn push(r: Router, url: string): Router = r\ncomponent App\n",
    );
    assert_eq!(warned(&own), [], "{own}");
    for verb in ["go", "push"] {
        let src = app(&format!(
            "    let n = push(nav, url)\n    nav = select({verb}(n, top(n).url), \"patterns\")\n"
        ));
        assert_eq!(warned(&src), [(SELECT.to_string(), 8)], "{src}");
        let src = app(&format!(
            "    nav = select({verb}(nav, top(back(nav)).url), \"patterns\")\n"
        ));
        assert_eq!(warned(&src), [(SELECT.to_string(), 7)], "{src}");
        let src = app(&format!(
            "    nav = select({verb}(nav, top(nav).url), \"patterns\")\n"
        ))
        .replace(
            "component App\n",
            "shape Location\n  id: number\n  url: string\nfn top(r: Router): Location = Location(id=0, url=\"/patterns/open\")\ncomponent App\n",
        );
        assert_eq!(warned(&src), [(SELECT.to_string(), 11)], "{src}");
    }
    for body in [
        "    nav = select(go(nav, top(nav).url), \"patterns\")\n",
        "    nav = select(push(nav, top(nav).url), \"patterns\")\n",
        "    let n = go(nav, top(nav).url)\n    nav = select(n, \"patterns\")\n",
        "    let n = push(nav, top(nav).url)\n    nav = select(n, \"patterns\")\n",
        "    nav = go(select(nav, \"patterns\"), url)\n",
        "    nav = open(nav, url)\n",
        "    let n = back(nav)\n    nav = select(n, \"today\")\n",
        // A binder of the same name hides the pushed `let`.
        "    let n = go(nav, url)\n    nav = match some(nav) { case some(n) => select(n, \"patterns\"), case none => n }\n",
        "    let n = go(nav, url)\n    match some(nav)\n      case some(n)\n        nav = select(n, \"patterns\")\n      case none\n        nav = n\n",
        // A choice is pushed only when every arm is.
        "    let on = url.startsWith(\"/patterns/\")\n    let n = on ? go(nav, url) : nav\n    nav = on ? n : select(n, \"patterns\")\n",
        // A `let` in another block.
        "    if url == \"/\"\n      let n = go(nav, url)\n      nav = n\n    else\n      let n = nav\n      nav = select(n, \"patterns\")\n",
    ] {
        assert_eq!(warned(&app(body)), [], "{body}");
    }
}

/// The euxy port: a template location is refused naming the `path()` that
/// spells it, a query concatenated after it.
#[test]
fn a_template_location_names_its_path() {
    let app = |location: &str| {
        format!(
            "routes nav\n  home \"/\"\n    day \"/day/:key\"\n    open \"/open\"\ncomponent App\n  state k = \"a\"\n  state ks = [\"a\"]\n  action visit\n    nav = push(nav, {location})\n  view\n    button press=visit\n      text \"Go\"\n"
        )
    };
    let refusal = |location: &str| contract::compile(&app(location)).unwrap_err();
    for (location, write) in [
        ("`/day/${k}`", "path(\"day\", k)"),
        (
            "`/open?u=${encodeURIComponent(k)}`",
            "path(\"open\") + `?u=${encodeURIComponent(k)}`",
        ),
        // A method and `length` as the author writes them.
        (
            "`/day/${k.trim().toLowerCase()}`",
            "path(\"day\", k.trim().toLowerCase())",
        ),
        ("`/day/${ks.length}`", "path(\"day\", ks.length)"),
    ] {
        let e = refusal(location);
        assert_eq!(e.id, "route-template", "{e}");
        let quoted = match write.contains('`') {
            true => format!("write `` {write} ``"),
            false => format!("write `{write}`"),
        };
        assert!(e.message.ends_with(&quoted), "{e}");
        // The repair compiles.
        if let Err(e) = contract::compile(&app(write)) {
            panic!("{write}: {e}");
        }
    }
    let src = app("`/day/${k}`").replace("state k = \"a\"", "state k = true");
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "route-template", "{e}");
    assert!(
        e.message.ends_with("write `` path(\"day\", `${k}`) ``"),
        "{e}"
    );
    let repaired = src.replace("`/day/${k}`", "path(\"day\", `${k}`)");
    if let Err(e) = contract::compile(&repaired) {
        panic!("{repaired}: {e}");
    }
    let unconverted = src.replace("`/day/${k}`", "path(\"day\", k)");
    assert_eq!(
        contract::compile(&unconverted).unwrap_err().id,
        "type-argument"
    );
    // No route, or text a repair could not write back as it reads: the hint alone.
    for location in [
        "`/elsewhere/${k}`",
        "`/day/\\${k}`",
        "`/day/${k}?q=\\\\`",
        "`/day/${k}?q=\\``",
        "`/day/${\"a\\\"b\"}`",
    ] {
        let e = refusal(location);
        assert_eq!(e.id, "route-template", "{location}: {e}");
        assert!(
            e.message.ends_with("would make another location)"),
            "{location}: {e}"
        );
    }
}
