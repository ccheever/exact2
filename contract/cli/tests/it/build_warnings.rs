//! Warning output and error-only CI gates.

use super::diagnostics::{diagnostics, App};
use serde_json::Value;

/// LLP 1116 D6: a warning is a diagnostic of severity `"warning"` after
/// every refusal, in the same array or on the same stream; it never fails
/// the build, and a refusal says `"severity": "error"`.
#[test]
fn warnings_follow_the_refusals_and_never_fail_the_build() {
    let app = App::new("warnings");
    let source = "shape Ack\n  ok: bool\ncomponent App\n  mutation wrote as shape Ack\n  action save\n    send wrote = keep(performanceNow())\n  view\n    button \"save\" press=save\n";
    app.write("app.contract", source);
    let warnings = diagnostics(&app.run(&["app.contract", "--json", "-o", "out.plan"]), 0);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(diagnostics(&app.run(&["app.contract", "--json", "--errors-only"]), 0).is_empty());
    assert_eq!(warnings[0]["id"], "type-performance-now-persisted");
    assert_eq!(warnings[0]["severity"], "warning");
    assert_eq!(warnings[0]["file"], "app.contract");
    assert_eq!(
        (&warnings[0]["line"], &warnings[0]["col"]),
        (&6.into(), &23.into())
    );
    assert!(app.0.join("out.plan").exists(), "a warning writes the plan");
    let human = app.run(&["app.contract"]);
    assert!(human.status.success(), "{human:?}");
    assert!(String::from_utf8_lossy(&human.stdout).contains("nodes,"));
    let stderr = String::from_utf8_lossy(&human.stderr);
    assert!(
        stderr.starts_with("app.contract:6:23 warning [type-performance-now-persisted] "),
        "{stderr}"
    );
    // Beside a refusal: the refusal first, the warning after, and the
    // refusal's exit status.
    app.write(
        "app.contract",
        &source.replace("button \"save\"", "button widht=3 \"save\""),
    );
    let errors = diagnostics(&app.run(&["app.contract", "--json", "--errors-only"]), 1);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["severity"], "error");
    let both = diagnostics(&app.run(&["app.contract", "--json"]), 1);
    let ids: Vec<(&Value, &Value)> = both.iter().map(|d| (&d["severity"], &d["id"])).collect();
    assert_eq!(
        ids,
        [
            (&"error".into(), &"lower-unknown-attr".into()),
            (&"warning".into(), &"type-performance-now-persisted".into())
        ]
    );
    let human = app.run(&["app.contract"]);
    assert_eq!(human.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&human.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines.len() == 2
            && lines[0].contains("[lower-unknown-attr]")
            && lines[1].contains(" warning ["),
        "{stderr}"
    );
}
