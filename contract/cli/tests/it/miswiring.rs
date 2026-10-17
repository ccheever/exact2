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
