//! `contract tamber check | eval | ir` (LLP 1121 D2): an app's design on its
//! own, found where the compiler finds it.

use std::process::Command;

const DESIGN: &str = "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  surface.card = light-dark(#ffffff, #000000)\n    when dark-style=dim: light-dark(#ffffff, #15202b)\n  brand = #c8102e\n";

fn app(tag: &str, design: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("contract-tamber-cli-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    text \"a\" background-color=\"surface.card\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("app.tamber"), design).unwrap();
    dir
}

fn contract(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn check_eval_and_ir_read_the_apps_design() {
    let dir = app("ok", DESIGN);
    let at = dir.to_str().unwrap();
    let (ok, out, _) = contract(&["tamber", "check", at]);
    assert!(
        ok && out.contains("2 tokens, 1 axis (2 tuples): checked"),
        "{out}"
    );
    let (ok, out, _) = contract(&["tamber", "eval", &format!("{at}/app.contract")]);
    assert!(ok, "{out}");
    assert_eq!(
        out,
        "surface.card = light-dark(#ffffffff, #000000ff)\nbrand = #c8102eff\n"
    );
    // The flag after the path or before it.
    for args in [
        ["tamber", "eval", at, "--mode", "dark-style=dim"],
        ["tamber", "eval", "--mode", "dark-style=dim", at],
    ] {
        let (ok, out, _) = contract(&args);
        assert!(
            ok && out.starts_with("surface.card = light-dark(#ffffffff, #15202bff)\n"),
            "{out}"
        );
    }
    let (ok, out, _) = contract(&["tamber", "ir", &format!("{at}/app.tamber")]);
    assert!(
        ok && out.contains("\"format\": 1") && out.contains("\"tuples\": 2"),
        "{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_writes_the_layout_and_check_writes_nothing() {
    let dir = app("fmt", "tamber t\ntheme\n    ink=#000000\n");
    let at = dir.to_str().unwrap();
    let file = dir.join("app.tamber");
    let (ok, _, err) = contract(&["tamber", "fmt", "--check", at]);
    assert!(!ok && err.contains("not in tamber fmt's layout"), "{err}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "tamber t\ntheme\n    ink=#000000\n"
    );
    let (ok, out, _) = contract(&["tamber", "fmt", at]);
    assert!(ok && out.contains("formatted"), "{out}");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "tamber t\n\ntheme\n  ink = #000000\n"
    );
    let (ok, _, _) = contract(&["tamber", "fmt", at, "--check"]);
    assert!(ok);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refusal_names_its_line_and_a_mode_is_axis_equals_value() {
    let dir = app("bad", "tamber t\n\ntheme\n  a = #zzz\n");
    let (ok, _, err) = contract(&["tamber", "check", dir.to_str().unwrap()]);
    assert!(!ok && err.contains("app.tamber:4:7: tamber-kind:"), "{err}");
    // What the compiler refuses of a design beyond the crate's check.
    let words = app("words", "tamber t\n\ntheme\n  top = #000000\n");
    let (ok, _, err) = contract(&["tamber", "check", words.to_str().unwrap()]);
    assert!(
        !ok && err.contains("app.tamber:4:3: lower-theme-name:"),
        "{err}"
    );
    let _ = std::fs::remove_dir_all(&words);
    let good = app("mode", DESIGN);
    let at = good.to_str().unwrap();
    for modes in [
        "dark-style:dim",
        "dark-style=grey",
        "dark-style=dim,dark-style=black",
    ] {
        let (ok, _, err) = contract(&["tamber", "eval", at, "--mode", modes]);
        assert!(
            !ok && err.contains("contract tamber eval"),
            "{modes}: {err}"
        );
    }
    let (ok, _, err) = contract(&["tamber", "check", std::env::temp_dir().to_str().unwrap()]);
    assert!(!ok && err.contains("has no design"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&good);
}
