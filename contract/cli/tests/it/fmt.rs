//! LLP 1035.005 D1's three proofs over every corpus file and every app:
//! `parse(fmt(src))` equals `parse(src)` up to spans and trivia,
//! `fmt(fmt(src)) == fmt(src)`, and the compiled plans are byte-identical.
//! No checked-in file is formatted: source snapshots use their original
//! path for import and asset resolution.

use contract_syntax::fmt::format;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every `.contract` in the corpus (including `use/`) and every app's.
fn sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["contract/corpus", "contract/corpus/use"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(repo().join(dir))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "contract"))
            .collect();
        files.sort();
        out.extend(files);
    }
    let mut apps: Vec<PathBuf> = std::fs::read_dir(repo().join("apps"))
        .unwrap()
        .map(|e| e.unwrap().path().join("app.contract"))
        .filter(|p| p.is_file())
        .collect();
    apps.sort();
    out.extend(apps);
    assert!(out.len() >= 24, "{} sources", out.len());
    out
}

/// The tree with every span and the trivia removed: `Debug` text with the
/// `Span { … }` fragments cut out.
fn shape(src: &str) -> String {
    let file = contract_syntax::parse(src).unwrap();
    let text = format!("{file:?}");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(i) = rest.find("Span {") {
        out.push_str(&rest[..i]);
        let close = rest[i..].find('}').unwrap();
        rest = &rest[i + close + 1..];
    }
    out.push_str(rest);
    out
}

#[test]
fn every_corpus_file_and_app_round_trips_through_the_printer() {
    let mut report = Vec::new();
    for path in sources().iter() {
        let name = path.strip_prefix(repo()).unwrap().display().to_string();
        let src = std::fs::read_to_string(path).unwrap();
        let formatted = format(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(shape(&src), shape(&formatted), "{name}: the tree changed");
        assert_eq!(
            format(&formatted).unwrap(),
            formatted,
            "{name}: not idempotent"
        );
        let over = src.lines().filter(|l| l.chars().count() > 100).count();
        let still_over = formatted
            .lines()
            .filter(|l| l.chars().count() > 100)
            .count();
        // A used file has no root of its own and does not compile alone;
        // every other source compiles to the same bytes after formatting.
        match contract::compile_path(path) {
            Ok(plan) => {
                let after = contract::compile_path_source(path, &formatted)
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(plan.encode(), after.encode(), "{name}: the plan changed");
                report.push(format!(
                    "{name}: byte-identical plan; {over} line(s) over 100 columns, {} after formatting; {} -> {} lines",
                    still_over,
                    src.lines().count(),
                    formatted.lines().count()
                ));
            }
            Err(e) => {
                assert!(
                    e.id == "analyze-root-props" && name.contains("use/"),
                    "{name}: {e}"
                );
                report.push(format!("{name}: a used file, tree and idempotence only"));
            }
        }
    }
    for line in &report {
        println!("{line}");
    }
    assert!(report.iter().filter(|l| l.contains("apps/")).count() >= 6);
}

#[test]
fn fmt_check_diffs_and_exits_non_zero_only_on_a_difference() {
    let dir = std::env::temp_dir().join(format!("exact-fmt-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.contract");
    std::fs::write(
        &file,
        "component A\n  view\n    text   \"a\"   font-size=12\n",
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", "--check"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let diff = String::from_utf8(out.stdout).unwrap();
    assert!(
        diff.contains("-    text   \"a\"   font-size=12\n"),
        "{diff}"
    );
    assert!(diff.contains("+    text \"a\" font-size=12\n"), "{diff}");
    // `--stdout` prints without touching the file; a bare `fmt` rewrites it.
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", "--stdout"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "component A\n  view\n    text \"a\" font-size=12\n"
    );
    assert!(std::fs::read_to_string(&file).unwrap().contains("text   "));
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("fmt")
        .arg(&file)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "component A\n  view\n    text \"a\" font-size=12\n"
    );
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", "--check"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fmt_check_keeps_distant_edits_local_without_rewriting_source() {
    let dir = std::env::temp_dir().join(format!("exact-fmt-hunks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.contract");
    let before = format!(
        "component A\n  view\n{}",
        (0..1000)
            .map(|n| {
                let gap = if n == 1 || n == 998 { "   " } else { " " };
                format!("    text{gap}\"row {n}\"\n")
            })
            .collect::<String>()
    );
    std::fs::write(&file, &before).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", "--check"])
        .arg(&file)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let diff = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        diff.lines().filter(|line| line.starts_with("@@")).count(),
        2
    );
    assert_eq!(
        diff.lines()
            .filter(|line| line.starts_with("-    text"))
            .count(),
        2
    );
    assert_eq!(
        diff.lines()
            .filter(|line| line.starts_with("+    text"))
            .count(),
        2
    );
    assert!(!diff.contains("row 500"));
    assert!(diff.lines().count() < 25);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn formatter_refuses_unknown_flags_extra_paths_and_invalid_source_without_writing() {
    let dir = std::env::temp_dir().join(format!("exact-fmt-refusal-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.contract");
    let original = "component A\n  view\n    text   \"a\"";
    std::fs::write(&file, original).unwrap();
    let path = file.to_str().unwrap();
    for args in [
        vec!["fmt", "--unknown", path],
        vec!["fmt", "--check", "--stdout", path],
        vec!["fmt", path, path],
        vec!["fmt", "--check"],
        vec!["fmt"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_contract"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    }
    let result = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", path, "--check"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stdout)
        .unwrap()
        .contains("\\ No newline at end of file"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
    let invalid = "component A\n  view\n    text \"a\" width=1\n      width=2\n";
    std::fs::write(&file, invalid).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", path])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("syntax-duplicate-attr"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), invalid);
    std::fs::remove_dir_all(&dir).unwrap();
}
