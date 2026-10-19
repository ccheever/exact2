//! LLP 1121 stage 11 B, round 1 (theme2's (a)): a design value naming a base
//! name bare (`card.pad: length = space.md`, a `when` arm too) that the design
//! leaves unset stays a per-host reference, never folded: each column reads
//! its own. One the design sets is its token, so the app's value wins through
//! the alias and an edit of it re-folds the alias. One test, its own process:
//! the column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::style::scale;
use exact_kernel::{Kernel, StyleId};
use exact_runner::{DataError, DataSource, Runner, Value};
use std::path::PathBuf;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, design: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-base-alias-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column testId=\"card\" padding=\"card.pad\"\n      text \"a\"\n").unwrap();
    dir.join("app.contract")
}

fn boot(path: &PathBuf) -> Runner<NoData> {
    let plan = contract::bake(contract::compile_path(path).unwrap(), NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn padding(r: &Runner<NoData>) -> String {
    let key = r.kernel().find_by_test_id("card")[0];
    format!(
        "{:?}",
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .computed(StyleId::PaddingTop)
    )
}

fn px(name: &str, column: Column) -> String {
    match scale::of(name, column).unwrap() {
        scale::ScaleValue::Px(n) => format!("({n:?})"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_alias_to_a_base_name_reads_each_columns_and_the_apps_own_live() {
    // Kept unfolded: the plan's token is the base name, as written.
    let alias = app("columns", "tamber t\n\naxes\n  tone = plain | wide\n\ntheme\n  card.pad: length = space.md\n    when tone=wide: space.lg\n");
    let plan = contract::compile_path(&alias).unwrap();
    let token = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == "card.pad")
        .unwrap();
    assert_eq!(plan.str(token.value), "space.md");
    // Each column its own: the neutral look's and the Mac's differ for space.md (16, 10).
    assert_ne!(px("space.md", Column::Neutral), px("space.md", Column::Mac));
    for column in [Column::Neutral, Column::Mac, Column::Ios] {
        set_column(column);
        let mut r = boot(&alias);
        assert!(
            padding(&r).contains(&px("space.md", column)),
            "{column:?}: {}",
            padding(&r)
        );
        // The `when` arm's base name, in the same column.
        r.set_mode("tone", "wide").unwrap();
        assert!(
            padding(&r).contains(&px("space.lg", column)),
            "{column:?}: {}",
            padding(&r)
        );
    }
    // The app's own `space.md` is its token: it wins through the alias, which
    // folds with it (as any token reference does), and an edit of it re-folds
    // the alias, which the design server's batch then carries.
    set_column(Column::Neutral);
    let fold = |space: &str| {
        let design =
            format!("tamber t\n\ntheme\n  space.md = {space}\n  card.pad: length = space.md\n");
        let folding = exact_tamber::check_design(
            exact_tamber::parse(&design).unwrap(),
            &contract_lower::tamber_vocab(),
        )
        .unwrap();
        let t = folding
            .tokens
            .iter()
            .find(|t| t.name == "card.pad")
            .unwrap();
        (t.values[0].css(), t.names.clone())
    };
    let (before, names) = fold("40px");
    assert_eq!(before, "40px");
    assert!(
        names.iter().any(|n| n == "space.md"),
        "an edit of space.md re-folds card.pad: {names:?}"
    );
    assert_eq!(fold("48px").0, "48px");
    let own = app(
        "own",
        "tamber t\n\ntheme\n  space.md = 40px\n  card.pad: length = space.md\n",
    );
    let r = boot(&own);
    assert!(padding(&r).contains("(40.0)"), "{}", padding(&r));
    set_column(Column::Web);
}
