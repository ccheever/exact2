//! @ref LLP 1060 — `t("key", name=value)` against `strings/<locale>.json`:
//! checked at compile, baked into the plan, the locale a slot the host's
//! place writes, so a switch re-renders exactly the texts `t` produced.
use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

struct AppDir(PathBuf);

impl AppDir {
    fn new(source: &str, tables: &[(&str, &str)]) -> AppDir {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "exact-contract-strings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("strings")).unwrap();
        std::fs::write(dir.join("app.contract"), source).unwrap();
        for (name, json) in tables {
            let path = dir.join(name);
            std::fs::write(path, json).unwrap();
        }
        AppDir(dir)
    }

    fn compile(&self) -> Result<exact_plan::Plan, Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|(plan, _)| plan)
    }

    fn refusals(&self) -> Vec<String> {
        self.compile()
            .expect_err("refused")
            .iter()
            .map(|e| e.id.clone())
            .collect()
    }
}

impl Drop for AppDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const APP: &str = "component App\n  state count = 2\n  action more writes count\n    count = count + 1\n  view\n    column\n      text t(\"title\") testId=\"title\"\n      text t(\"greeting\", name=\"Ada\", count=count) testId=\"greeting\"\n      text \"fixed\" testId=\"fixed\"\n      text t(\"only-base\") testId=\"only\"\n";

const EN: &str = r#"{"title": "Journal", "greeting": "Hi {name}, {count} entries", "only-base": "Base only", "unused": "Never baked"}"#;
const EN_GB: &str = r#"{"title": "Diary"}"#;
const FR: &str = r#"{"title": "Journal intime", "greeting": "{count} entrées, {name}"}"#;

fn tables() -> Vec<(&'static str, &'static str)> {
    vec![
        ("strings/en.json", EN),
        ("strings/en-GB.json", EN_GB),
        ("strings/fr.json", FR),
    ]
}

fn text(r: &Runner<NoData>, id: &str) -> String {
    let key = r.kernel().find_by_test_id(id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props.str(PropId::Text).unwrap_or("").to_string()
}

fn boot(plan: exact_plan::Plan) -> Runner<NoData> {
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn tables_are_baked_base_first_with_only_the_keys_t_names() {
    let app = AppDir::new(APP, &tables());
    let plan = app.compile().unwrap();
    let names: Vec<&str> = plan.locales.iter().map(|l| plan.str(l.name)).collect();
    assert_eq!(names, ["en", "en-GB", "fr"]);
    let base = &plan.locales[0];
    let keys: Vec<&str> = base
        .texts
        .iter()
        .map(|t| plan.str(plan.text(t).key))
        .collect();
    assert_eq!(keys, ["greeting", "only-base", "title"]);
    let slot = plan.slot(plan.locale.expect("the locale slot"));
    assert_eq!(plan.str(slot.name), "#locale");
    // Encode and decode: the tables and the header field survive, canonically.
    let bytes = plan.encode();
    let decoded = exact_plan::Plan::decode(&bytes).unwrap();
    assert_eq!(decoded, plan);
    assert_eq!(decoded.encode(), bytes);
    // Compiling twice is byte-identical.
    assert_eq!(app.compile().unwrap().encode(), bytes);
}

#[test]
fn data_only_tables_bake_names_but_no_texts() {
    let app = AppDir::new(
        "component App\n  view\n    text \"plain\"\n",
        &[("strings/en.json", EN)],
    );
    let plan = app.compile().unwrap();
    assert_eq!(plan.locales.len(), 1);
    assert!(plan.texts.is_empty());
    assert!(plan.locale.is_some());
}

#[test]
fn the_base_shows_until_the_host_says_and_a_switch_remeasures_text() {
    let app = AppDir::new(APP, &tables());
    let mut r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "title"), "Journal");
    assert_eq!(text(&r, "greeting"), "Hi Ada, 2 entries");
    // Language-only fallback, then the base for a key the table lacks.
    let receipt = r
        .set_place("fr-CA", "America/Toronto", None)
        .unwrap()
        .unwrap();
    assert_eq!(text(&r, "title"), "Journal intime");
    assert_eq!(text(&r, "greeting"), "2 entrées, Ada");
    assert_eq!(text(&r, "only"), "Base only");
    assert_eq!(text(&r, "fixed"), "fixed");
    let mut touched = receipt.touched.clone();
    touched.sort();
    let mut t_texts: Vec<_> = ["title", "greeting", "only", "fixed"]
        .iter()
        .map(|id| r.kernel().find_by_test_id(id)[0])
        .collect();
    t_texts.sort();
    assert_eq!(touched, t_texts);
    // A placeholder's value is ordinary state.
    r.act("more", vec![]).unwrap();
    assert_eq!(text(&r, "greeting"), "3 entrées, Ada");
    // An exact match beats the language; a key it lacks falls to the base.
    r.set_place("en-GB", "Europe/London", None)
        .unwrap()
        .unwrap();
    assert_eq!(text(&r, "title"), "Diary");
    assert_eq!(text(&r, "greeting"), "Hi Ada, 3 entries");
    // A locale that resolves to the same table commits nothing.
    assert!(r
        .set_place("EN-gb", "Europe/London", None)
        .unwrap()
        .is_none());
    assert_eq!(r.slot("#locale"), Some(&Value::str("en-GB")));
    // No table for the language: the base.
    r.set_place("de-DE", "Europe/Berlin", None)
        .unwrap()
        .unwrap();
    assert_eq!(text(&r, "title"), "Journal");
}

#[test]
fn a_changed_place_that_resolves_to_the_same_table_rerenders_nothing() {
    let app = AppDir::new(APP, &tables());
    let mut r = boot(app.compile().unwrap());
    // No exactTime resource and the same table: no commit at all.
    assert!(r
        .set_place("en-US", "America/New_York", None)
        .unwrap()
        .is_none());
    assert_eq!(text(&r, "title"), "Journal");
}

#[test]
fn an_initializer_may_call_t() {
    let app = AppDir::new(
        "component App\n  state label = t(\"title\")\n  view\n    text label testId=\"label\"\n",
        &[("strings/en.json", EN)],
    );
    let r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "label"), "Journal");
}

#[test]
fn the_manifest_names_the_base() {
    let app = AppDir::new(
        "component App\n  view\n    text t(\"title\") testId=\"title\"\n",
        &[
            (
                "app.json",
                r#"{"name":"S","app":{"id":"com.exact.s","name":"S"},"strings":{"base":"fr"}}"#,
            ),
            ("strings/fr.json", r#"{"title": "Journal intime"}"#),
        ],
    );
    let r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "title"), "Journal intime");
}

#[test]
fn the_base_locale_matches_without_case_and_is_baked_first() {
    let app = AppDir::new(
        APP,
        &[
            (
                "app.json",
                r#"{"name":"S","app":{"id":"com.exact.s","name":"S"},"strings":{"base":"pt-BR"}}"#,
            ),
            ("strings/pt-br.json", EN),
            ("strings/fr.json", FR),
        ],
    );
    let plan = app.compile().unwrap();
    assert_eq!(plan.str(plan.locales[0].name), "pt-br");
    let mut r = boot(plan);
    assert_eq!(text(&r, "title"), "Journal");
    r.set_place("fr", "Europe/Paris", None).unwrap();
    assert_eq!(text(&r, "only"), "Base only");
    r.set_place("PT-br", "America/Sao_Paulo", None).unwrap();
    assert_eq!(text(&r, "title"), "Journal");
}

#[test]
fn interpolation_refuses_expansion_past_the_vms_string_cap() {
    let src = "component App\n  state label = \"\"\n  action expand(s: string) writes label\n    label = t(\"large\", name=s)\n  view\n    text label\n";
    let table = serde_json::json!({ "large": "{name}".repeat(1025) }).to_string();
    let app = AppDir::new(src, &[("strings/en.json", &table)]);
    let mut r = boot(app.compile().unwrap());
    let error = r
        .act("expand", vec![Value::str(&"x".repeat(64 * 1024))])
        .unwrap_err();
    assert!(
        matches!(
            error,
            exact_runner::RunnerError::Trap(exact_runner::vm::Trap::StringTooLong { .. })
        ),
        "{error:?}"
    );
    assert_eq!(r.slot("label"), Some(&Value::str("")));
}

#[test]
fn each_call_site_refusal_has_its_id() {
    let cases = [
        ("text t(\"titel\")", "type-strings-unknown-key"),
        (
            "text t(\"greeting\", name=\"Ada\")",
            "type-strings-placeholder",
        ),
        ("text t(\"title\", name=\"Ada\")", "type-strings-argument"),
        ("text t(\"title\", \"Ada\")", "type-strings-argument"),
        (
            "text t(\"greeting\", name=\"Ada\", count=none)",
            "type-strings-argument",
        ),
        ("text t(\"ti\" + \"tle\")", "type-strings-key"),
    ];
    for (line, id) in cases {
        let app = AppDir::new(
            &format!("component App\n  view\n    {line}\n"),
            &[("strings/en.json", EN)],
        );
        assert_eq!(app.refusals(), [id], "{line}");
    }
    // A typo gets a suggestion.
    let app = AppDir::new(
        "component App\n  view\n    text t(\"titel\")\n",
        &[("strings/en.json", EN)],
    );
    let message = app.compile().unwrap_err()[0].message.clone();
    assert!(message.contains("did you mean \"title\""), "{message}");
    // Text without a path has no tables.
    assert_eq!(
        contract::compile("component App\n  view\n    text t(\"title\")\n")
            .unwrap_err()
            .id,
        "type-strings-missing"
    );
}

#[test]
fn every_table_refusal_is_reported_in_one_run() {
    let app = AppDir::new(
        "component App\n  view\n    text \"x\"\n",
        &[
            ("strings/en.json", EN),
            (
                "strings/fr.json",
                r#"{"title": "{who} Journal", "gone": "Parti"}"#,
            ),
            ("strings/es.json", r#"{"title": 3}"#),
            ("strings/en_US.json", "{}"),
        ],
    );
    let mut ids = app.refusals();
    ids.sort();
    assert_eq!(
        ids,
        [
            "strings-locale",
            "strings-placeholder",
            "strings-table",
            "strings-unknown-key"
        ]
    );
    let app = AppDir::new(
        "component App\n  view\n    text \"x\"\n",
        &[("strings/fr.json", FR)],
    );
    assert_eq!(app.refusals(), ["strings-base-missing"]);
}

/// A child component's loop variable named like the string function does not
/// hide it: expansion renames the child's locals, never a call's name.
#[test]
fn a_loop_variable_named_t_leaves_t_callable_in_a_child() {
    let app = AppDir::new(
        "shape Row\n  id: string\ncomponent App\n  resource rows = rows() as shape list<Row>\n  view\n    Rows(rows=rows)\ncomponent Rows\n  props\n    rows: list<Row>\n  view\n    column\n      each t in rows key=t.id\n        text t(\"hi\") testId=t.id\n",
        &[("strings/en.json", r#"{"hi": "Hello"}"#)],
    );
    assert!(app.compile().is_ok(), "{:?}", app.refusals());
}

#[test]
fn the_default_agent_place_still_selects_its_table_when_the_base_differs() {
    let app = AppDir::new(
        "component App\n  view\n    text t(\"title\") testId=\"title\"\n",
        &[
            ("strings/en.json", r#"{"title":"Base journal"}"#),
            ("strings/en-US.json", r#"{"title":"Journal"}"#),
        ],
    );
    let mut r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "title"), "Base journal");
    assert!(r.set_place("en-US", "UTC", Some(0.0)).unwrap().is_some());
    assert_eq!(text(&r, "title"), "Journal");
    assert!(r.set_place("en-US", "UTC", Some(0.0)).unwrap().is_none());
}

#[test]
fn mf2_constructs_are_refused_by_name_even_without_t() {
    for (message, construct) in [
        (".match $n\none {{One}}\n* {{Many}}", ".match"),
        (
            ".input {$n :number}\n.match $n\none {{One}}\n* {{Many}}",
            ".input",
        ),
        (".local $x = {$n}\n{{Hi}}", ".local"),
        ("{$n :number}", ":number"),
        ("{#bold}Hi{/bold}", "markup"),
        ("{n, plural, one {One} other {Many}}", "plural"),
        ("{$n @foo}", "attributes"),
        ("{{Hello}}", "quoted pattern"),
        ("{||}", "literal"),
        ("bad }", "brace"),
        (r"bad \q", "escape"),
    ] {
        let json = serde_json::json!({"message": message}).to_string();
        let app = AppDir::new(
            "component App\n  view\n    text \"plain\"\n",
            &[("strings/en.json", &json)],
        );
        let errors = app.compile().expect_err(construct);
        assert!(
            errors.iter().any(|e| e.id == "strings-message"
                && e.message.contains(construct)
                && e.message.contains("message")
                && e.file.as_ref().unwrap().ends_with("en.json")),
            "{errors:?}"
        );
    }
}

#[test]
fn mf2_variables_and_escapes_share_the_checked_runtime_grammar() {
    let json = serde_json::json!({"hi": r"Hi {$name}: \{name\} \\ {name}"}).to_string();
    let app = AppDir::new(
        "component App\n  view\n    text t(\"hi\", name=\"Ada\") testId=\"hi\"\n",
        &[("strings/en.json", &json)],
    );
    let r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "hi"), r"Hi Ada: {name} \ Ada");
}

#[test]
fn resolved_locale_is_answered_even_for_tables_used_only_by_data() {
    let app = AppDir::new("shape Time\n  locale: string\n  resolvedLocale: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text `${time.locale}|${time.resolvedLocale}` testId=\"place\"\n", &tables());
    let mut r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "place"), "en-US|en");
    r.set_place("fr-CA", "UTC", None).unwrap();
    assert_eq!(text(&r, "place"), "fr-CA|fr");
    r.set_place("ar", "UTC", None).unwrap();
    assert_eq!(text(&r, "place"), "ar|en");
}

#[test]
fn cldr_direction_follows_the_resolved_script_and_css_overrides_still_win() {
    let app = AppDir::new("component App\n  view\n    column testId=\"root\"\n      text t(\"title\") testId=\"title\"\n      text \"CSS\" direction=\"ltr\" testId=\"override\"\n", &[
        ("strings/en.json", EN), ("strings/ar.json", EN),
        ("strings/ar-Latn.json", EN), ("strings/he.json", EN),
        ("strings/fa.json", EN), ("strings/ur.json", EN),
        ("strings/az-IR.json", EN), ("strings/pa-PK.json", EN),
    ]);
    let mut r = boot(app.compile().unwrap());
    for (locale, lang, dir) in [
        ("ar-EG", "ar", "rtl"),
        ("ar-Latn", "ar-Latn", "ltr"),
        ("he", "he", "rtl"),
        ("fa", "fa", "rtl"),
        ("ur", "ur", "rtl"),
        ("az-IR", "az-IR", "rtl"),
        ("pa-PK", "pa-PK", "rtl"),
        ("de", "en", "ltr"),
    ] {
        r.set_place(locale, "UTC", None).unwrap();
        assert_eq!(r.resolved_locale(), lang);
        assert_eq!(r.direction(), dir, "{locale}");
        for id in ["root", "title", "override"] {
            let node = r
                .kernel()
                .node_by_key(r.kernel().find_by_test_id(id)[0])
                .unwrap();
            let want = if id != "override" && dir == "rtl" {
                exact_kernel::Direction::Rtl
            } else {
                exact_kernel::Direction::Ltr
            };
            assert_eq!(
                node.computed_style(exact_kernel::StyleMask::INHERITED)
                    .direction,
                want,
                "{locale}: {id}"
            );
        }
    }
    let state = exact_runner::agent::state(&r);
    assert!(
        state.contains(r#""language":{"lang":"en","dir":"ltr"}"#),
        "{state}"
    );
    assert!(r.set_place("not a locale", "UTC", None).is_err());
    assert_eq!(r.resolved_locale(), "en");
}

#[test]
fn the_default_place_reanswers_resolved_locale_when_its_table_changes() {
    let app = AppDir::new("shape Time\n  resolvedLocale: string\ncomponent App\n  resource time = exactTime() as shape Time\n  view\n    text time.resolvedLocale testId=\"locale\"\n", &[
        ("strings/en.json", EN), ("strings/en-US.json", EN),
    ]);
    let mut r = boot(app.compile().unwrap());
    assert_eq!(text(&r, "locale"), "en");
    r.set_place("en-US", "UTC", None).unwrap();
    assert_eq!(text(&r, "locale"), "en-US");
}
