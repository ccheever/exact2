//! `lower-row-lookup` (LLP 1116.016 D4): a row of the app's records that
//! searches another table of them.

const ROW_LOOKUP: &str = "lower-row-lookup";

/// The sites `src` is warned at with `lower-row-lookup`.
fn lookups(src: &str) -> Vec<u32> {
    let (result, warnings) = contract::compile_warned(src);
    if let Err(e) = result {
        panic!("a warning never fails a build: {e}\n{src}");
    }
    warnings
        .iter()
        .filter(|w| w.id == ROW_LOOKUP)
        .map(|w| w.span.line)
        .collect()
}

/// Users and posts, a view at line 11 on.
fn app(view: &str) -> String {
    let view: String = view.lines().map(|l| format!("      {l}\n")).collect();
    format!(
        "shape User\n  id: string\n  name: string\nshape Post\n  id: string\n  userId: string\n  tags: string[]\ncomponent App\n  state users = [User(id=\"u\", name=\"Ada\")]\n  state posts = [Post(id=\"p\", userId=\"u\", tags=[\"a\"])]\n  state categories = [\"a\", \"b\"]\n  view\n    column\n{view}"
    )
}

#[test]
fn a_row_that_searches_another_table_is_named_with_the_join() {
    let src = app(
        "for p in posts key=p.id\n  text (match first(users.filter(u => u.id == p.userId)) { case some(u) => u.name, case none => \"\" })",
    );
    assert_eq!(lookups(&src), [15]);
    let (_, warnings) = contract::compile_warned(&src);
    let w = warnings.iter().find(|w| w.id == ROW_LOOKUP).unwrap();
    assert!(
        w.message
            .starts_with("each row of `posts` searches `users` (`.filter`)")
            && w.message.contains("join it in the data module"),
        "{}",
        w.message
    );
    // A function the roster writes as a call is named as one (LLP 1116.016 §5).
    let sorted =
        app("for p in posts key=p.id\n  text sortBy(users, u => u.name).length.toString()");
    let (_, warnings) = contract::compile_warned(&sorted);
    let w = warnings.iter().find(|w| w.id == ROW_LOOKUP).unwrap();
    assert!(
        w.message
            .starts_with("each row of `posts` searches `users` (`sortBy(…)`)"),
        "{}",
        w.message
    );
    // A nested `for` over the other table walks it in every row too.
    let nested = app("for p in posts key=p.id\n  for u in users key=u.id\n    text u.name");
    assert_eq!(lookups(&nested), [15]);
}

#[test]
fn a_row_s_own_lists_grouping_by_strings_and_lists_of_ids_are_not_named() {
    // The row's own list.
    assert_eq!(
        lookups(&app("for p in posts key=p.id\n  text p.tags.join(\", \")")),
        Vec::<u32>::new()
    );
    // One section per category, its posts filtered by it: grouping by a short key.
    assert_eq!(
        lookups(&app(
            "for c in categories key=c\n  text `${posts.filter(p => p.tags.includes(c)).length}`"
        )),
        Vec::<u32>::new()
    );
    // A list of strings is a set of choices or ids, not a table.
    assert_eq!(
        lookups(&app(
            "for p in posts key=p.id\n  text (categories.includes(p.id) ? \"★\" : \"\")"
        )),
        Vec::<u32>::new()
    );
    // Outside any row, the same search is one evaluation.
    assert_eq!(
        lookups(&app(
            "text `${users.filter(u => u.name == \"Ada\").length}`"
        )),
        Vec::<u32>::new()
    );
}

#[test]
fn a_router_s_tabs_are_screens_not_rows() {
    let src = "shape User\n  id: string\nroutes nav\n  tab feed \"/\"\n  tab me \"/me\"\ncomponent App\n  state users = [User(id=\"u\")]\n  view\n    main\n      for t in nav.tabs key=t.name\n        text `${users.filter(u => u.id == t.name).length}`\n";
    assert_eq!(lookups(src), Vec::<u32>::new());
}

/// `lower-key-position` (LLP 1142 D6): a key that reads the row's position.
#[test]
fn a_key_that_reads_the_position_is_named_and_one_that_shadows_it_is_not() {
    let keyed = |view: &str| -> Vec<u32> {
        let (result, warnings) = contract::compile_warned(&app(view));
        assert!(result.is_ok(), "a warning never fails a build");
        warnings
            .iter()
            .filter(|w| w.id == "lower-key-position")
            .map(|w| w.span.line)
            .collect()
    };
    assert_eq!(keyed("for p, i in posts key=i\n  text p.id"), [14]);
    assert_eq!(
        keyed("for p, i in posts key=`${p.userId}-${i}`\n  text p.id"),
        [14]
    );
    assert!(keyed("for p, i in posts key=p.id\n  text `${i}`").is_empty());
    let (_, warnings) = contract::compile_warned(&app("for p, i in posts key=i\n  text p.id"));
    let w = warnings
        .iter()
        .find(|w| w.id == "lower-key-position")
        .unwrap();
    assert!(
        w.message
            .starts_with("this key reads `i`, the row's position"),
        "{}",
        w.message
    );
}
