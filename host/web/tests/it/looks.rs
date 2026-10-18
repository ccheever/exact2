//! LLP 1121 D8 (stage 8 B): the wasm host writes a node's `:hover` and
//! `:focus-visible` looks as page rules (`looks`), again when a token they
//! read moves live; `:active` is the node's press paint (its scale, its
//! background, its absolute opacity). A node with no looks writes none.

use exact_runner::{design, DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

const VIEW: &str = "component App\n  state n = 0\n  action go\n    n = n + 1\n  view\n    column\n      column testId=\"card\" press=go background-color=\"#ffffff\"\n        :active\n          background-color = \"#0000ff\"\n          opacity = 0.6\n          -exact-press-scale = 0.95\n        :hover\n          border-color = \"#00ff00\"\n        :focus-visible\n          background-color = \"accent.soft\"\n        text \"c\"\n      column testId=\"plain\" press=go\n        text \"p\"\n";

fn plan(dir: &std::path::Path, soft: &str) -> exact_plan::Plan {
    std::fs::write(
        dir.join("app.tamber"),
        format!("tamber t\n\ntheme\n  accent.soft = {soft}\n"),
    )
    .unwrap();
    std::fs::write(dir.join("app.contract"), VIEW).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e:?}"))
}

/// The op that creates the element with `test_id`, to the next op.
fn created<'a>(batch: &'a str, test_id: &str) -> &'a str {
    let at = batch
        .find(&format!("\"data-testid\":\"{test_id}\""))
        .unwrap_or_else(|| panic!("no {test_id} in {batch}"));
    let start = batch[..at].rfind("{\"op\":").unwrap();
    let end = batch[at..].find("{\"op\":").map_or(batch.len(), |n| at + n);
    &batch[start..end]
}

#[test]
fn a_nodes_looks_are_page_rules_that_follow_a_live_token() {
    let dir = std::env::temp_dir().join(format!("exact-web-looks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = plan(&dir, "#eeeeee");
    let b = plan(&dir, "#ff0000");
    let _ = std::fs::remove_dir_all(&dir);
    let (mut host, boot) = Host::boot(&a.encode(), NoData, Default::default(), "/").unwrap();
    let card = created(&boot, "card");
    assert!(
        card.contains("--exact-press:0.95"),
        "the press's scale: {card}"
    );
    assert!(
        card.contains("--exact-press-opacity-to:0.6"),
        "its absolute opacity: {card}"
    );
    assert!(
        card.contains("--exact-press-background-color:rgba(0,0,255,1)"),
        "{card}"
    );
    let plain = created(&boot, "plain");
    assert!(!plain.contains("--exact-press"), "{plain}");
    let looks: Vec<&str> = boot
        .match_indices("{\"op\":\"looks\"")
        .map(|(at, _)| &boot[at..])
        .collect();
    assert_eq!(looks.len(), 1, "only the card has looks: {boot}");
    let rules = looks[0];
    let hover = rules.find(":hover").expect("a hover rule");
    let focus = rules.find(":focus-visible").expect("a focus rule");
    assert!(hover < focus, "the focus look wins: {rules}");
    assert!(
        !rules.contains("data-pressed"),
        "`:active` is no look: {rules}"
    );
    assert!(
        rules.contains("background-color:rgba(238,238,238,1)!important"),
        "{rules}"
    );
    assert!(
        rules.contains("border-top-color:rgba(0,255,0,1)!important"),
        "{rules}"
    );
    let batch =
        design::batch(&design::shape(&a), &design::tokens(&a), &design::tokens(&b)).unwrap();
    let out = host.apply_live(&batch.json());
    assert!(out.starts_with("{\"live\":\"applied\""), "{out}");
    assert!(
        out.contains("{\"op\":\"looks\""),
        "the look is written again: {out}"
    );
    assert!(
        out.contains("background-color:rgba(255,0,0,1)!important"),
        "{out}"
    );
}
