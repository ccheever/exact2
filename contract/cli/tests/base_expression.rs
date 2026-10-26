//! LLP 1121 D11, stage 12 B2: a length token over base names the design
//! leaves unset (`card.pad: length = calc(space.md * 2 + 4px)`) is read in
//! each host's column, its bases substituted and the whole folded by the
//! kernel's `calc()`: a row and a native binding alike, a `when` arm too,
//! the terminal's cells as px. So is a shadow token's length over one
//! (stage 12 B3). One test, its own process: the column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::style::scale;
use exact_kernel::{Kernel, StyleId};
use exact_runner::tamber::{Length, Token};
use exact_runner::{DataError, DataSource, Place, Runner, Value};
use std::path::{Path, PathBuf};

const CARD_PAD: Token<Length> = Token::new("card.pad");

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn app(name: &str, design: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "exact2-base-expression-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  view\n    column testId=\"card\" padding=\"card.pad\" box-shadow=\"card.shadow\"\n      text \"a\"\n").unwrap();
    dir.join("app.contract")
}

fn boot(path: &Path) -> Runner<NoData> {
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

fn shadow(r: &Runner<NoData>) -> String {
    let key = r.kernel().find_by_test_id("card")[0];
    format!(
        "{:?}",
        r.kernel()
            .node_by_key(key)
            .unwrap()
            .computed(StyleId::BoxShadow)
    )
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

#[test]
fn an_expression_reads_each_columns_bases() {
    const EXPR: &str = "calc(space.md * 2 + 4px)";
    const ARM: &str = "min(space.lg, 12px)";
    let path = app(
        "columns",
        &format!("tamber t\n\naxes\n  tone = plain | wide\n\ntheme\n  card.pad: length = {EXPR}\n    when tone=wide: {ARM}\n  card.shadow: box-shadow = 0 space.md calc(space.sm * 2) #000\n"),
    );
    let plan = contract::compile_path(&path).unwrap();
    let token = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == "card.pad")
        .unwrap();
    assert_eq!(plan.str(token.value), EXPR);
    // The neutral look's and the Mac's differ for space.md.
    assert_ne!(
        scale::evaluate(EXPR, Column::Neutral),
        scale::evaluate(EXPR, Column::Mac)
    );
    for column in [
        Column::Neutral,
        Column::Mac,
        Column::Ios,
        Column::Material,
        Column::Terminal,
    ] {
        set_column(column);
        let mut r = boot(&path);
        let want = |expr: &str| scale::evaluate(expr, column).unwrap();
        let want_px = want;
        // A native binding reads it as a row does.
        assert_eq!(
            r.tamber().get(CARD_PAD, Place::Root),
            Some(f64::from(want(EXPR))),
            "{column:?}"
        );
        // A row: its px (the terminal's then rounded to cells at layout,
        // as every space there is).
        if column != Column::Terminal {
            assert!(
                padding(&r).contains(&format!("({:?})", want(EXPR))),
                "{column:?}: {}",
                padding(&r)
            );
        }
        // A shadow over base names: each length the column's px.
        let text = plan
            .theme
            .iter()
            .find(|t| plan.str(t.name) == "card.shadow")
            .map(|t| plan.str(t.value).to_string())
            .unwrap();
        let px = scale::fold_lengths(&text, |n| scale::of(n, column).map(scale::css)).unwrap();
        let drawn = exact_kernel::style::BoxShadows::parse(&px)
            .unwrap()
            .shadows()[0];
        assert_eq!(drawn.offset.y, want_px("calc(space.md)"), "{column:?}");
        assert!(
            shadow(&r).contains(&format!("{:?}", drawn.offset))
                && shadow(&r).contains(&format!("blur: {:?}", want_px("calc(space.sm * 2)"))),
            "{column:?}: {}",
            shadow(&r)
        );
        r.set_mode("tone", "wide").unwrap();
        assert_eq!(
            r.tamber().get(CARD_PAD, Place::Root),
            Some(f64::from(want(ARM))),
            "{column:?}"
        );
        // An edit to another expression: the row and the binding read it.
        r.set_mode("tone", "plain").unwrap();
        r.set_token(
            "card.pad",
            "calc(space.sm * 3)",
            &["calc(space.sm * 3)", ARM],
        )
        .unwrap();
        assert_eq!(
            r.tamber().get(CARD_PAD, Place::Root),
            Some(f64::from(want("calc(space.sm * 3)"))),
            "{column:?}"
        );
        if column != Column::Terminal {
            assert!(
                padding(&r).contains(&format!("({:?})", want("calc(space.sm * 3)"))),
                "{column:?}: {}",
                padding(&r)
            );
        }
    }
    // The app's own space.md is its token: the expression folds with it.
    set_column(Column::Neutral);
    let own = app(
        "own",
        &format!("tamber t\n\ntheme\n  space.md = 40px\n  card.pad: length = {EXPR}\n  card.shadow: box-shadow = 0 space.md 1px #000\n"),
    );
    let plan = contract::compile_path(&own).unwrap();
    let token = plan
        .theme
        .iter()
        .find(|t| plan.str(t.name) == "card.pad")
        .unwrap();
    assert_eq!(plan.str(token.value), "84px");
    let r = boot(&own);
    assert!(padding(&r).contains("(84.0)"), "{}", padding(&r));
    assert!(shadow(&r).contains("y: 40.0"), "{}", shadow(&r));
    set_column(Column::Web);
}
