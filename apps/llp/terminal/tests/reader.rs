//! The reader run as the agent runs it: the binary, headless, on a corpus
//! of three documents.

use std::path::PathBuf;
use std::process::Command;

const ALPHA: &str = "# LLP 0001: Alpha\n\n**Type:** RFC\n**Status:** Draft\n**Author:** Someone\n\n## Summary\n\nSee LLP 0002 for the beta, and [the site](https://example.com/llp). A needle is here.\n\n## Decisions\n\n| # | Decision |\n|---|---|\n| D1 | The first decision, which is long |\n| D2 | Another |\n\n```rust\nlet needle = 1;\n```\n\n## After\n\nOne.\n\nTwo.\n\nThree.\n\nFour.\n\nFive.\n\nSix.\n\nSeven.\n";
const BETA: &str =
    "# LLP 0002: Beta\n\n**Type:** Spec\n**Status:** Accepted\n\n## Body\n\nNo pins in this one.\n";
const GAMMA: &str =
    "# LLP 0002.000: Gamma\n\n**Type:** Plan\n**Status:** Draft\n\nA needle, and another needle.\n";

fn corpus(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llp-reader-{}-{name}/llp", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in [
        ("0001-alpha.rfc.md", ALPHA),
        ("0002-beta.spec.md", BETA),
        ("0002.000-gamma.plan.md", GAMMA),
    ] {
        std::fs::write(dir.join(file), text).unwrap();
    }
    dir
}

/// Run the reader on a fresh corpus; each `print` is one screen.
fn run(name: &str, ops: &[&str]) -> Vec<String> {
    run_err(name, ops).0
}

/// [`run`], and what the reader said on stderr.
fn run_err(name: &str, ops: &[&str]) -> (Vec<String>, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_llp"))
        .arg(corpus(name))
        .args(["--size", "80x20", "until", "LLP 0001"])
        .args(ops)
        .output()
        .expect("runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}\n{text}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<&str> = text.lines().collect();
    let screens = lines.chunks(20).map(|c| c.join("\n")).collect();
    (screens, String::from_utf8_lossy(&out.stderr).into_owned())
}

/// The test id of the link run whose text is `text`, from the tree.
fn link(name: &str, text: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_llp"))
        .arg(corpus(name))
        .args(["--size", "80x20", "until", "LLP 0001", "tree"])
        .output()
        .expect("runs");
    let tree = String::from_utf8_lossy(&out.stdout).into_owned();
    let line = tree
        .lines()
        .find(|l| l.contains("Run #link-") && l.contains(&format!("{text:?}")))
        .unwrap_or_else(|| panic!("no link {text:?} in:\n{tree}"));
    let at = line.find("#link-").unwrap() + 1;
    line[at..].split_whitespace().next().unwrap().to_string()
}

#[test]
fn opens_the_first_document_with_its_header_as_fields() {
    let screens = run("open", &["print"]);
    let s = &screens[0];
    assert!(s.lines().next().unwrap().contains("LLP 0001  Alpha"), "{s}");
    assert!(s.contains("Status       Draft"), "{s}");
    assert!(s.contains("Author       Someone"), "{s}");
    // The table's columns are as wide as their cells, not equal.
    let row = s.lines().find(|l| l.contains("D1")).expect("a table row");
    assert!(row.find("The first").unwrap() < 20, "{row}");
}

#[test]
fn find_walks_the_matches_in_the_page() {
    let screens = run(
        "find",
        &[
            "key", "/", "type", "find", "needle", "key", "Enter", "print", "key", "n", "print",
            "key", "Escape", "print",
        ],
    );
    assert!(screens[0].contains("1/2 “needle”"), "{}", screens[0]);
    assert!(screens[1].contains("2/2 “needle”"), "{}", screens[1]);
    assert!(!screens[2].contains("“needle”"), "{}", screens[2]);
}

#[test]
fn the_tree_filters_the_corpus_and_opens_with_the_find() {
    let screens = run(
        "tree",
        &[
            "key",
            "t",
            "key",
            "/",
            "type",
            "tree-filter",
            "needle",
            "print",
            "key",
            "ArrowDown",
            "key",
            "ArrowDown",
            "key",
            "Enter",
            "wait",
            "200",
            "print",
        ],
    );
    let tree = &screens[0];
    assert!(tree.contains("2 of 3 contain “needle”"), "{tree}");
    assert!(!tree.contains("Beta"), "{tree}");
    let opened = &screens[1];
    assert!(opened.lines().next().unwrap().contains("Gamma"), "{opened}");
    assert!(
        opened.contains("1/2 “needle”"),
        "the filter came along as the find:\n{opened}"
    );
}

#[test]
fn links_and_neighbours_open_documents() {
    let screens = run(
        "links",
        &[
            "key", "l", "print", "key", "Enter", "wait", "200", "print", "key", "]", "wait", "200",
            "print",
        ],
    );
    assert!(
        screens[0].contains("LLP 0002") && screens[0].contains("Beta"),
        "{}",
        screens[0]
    );
    // LLP 0002 and the site: its own `LLP 0001` (its title) is no link.
    assert!(
        screens[0].contains("Links (2)"),
        "its own number is not a link: {}",
        screens[0]
    );
    assert!(
        screens[1]
            .lines()
            .next()
            .unwrap()
            .contains("LLP 0002  Beta"),
        "{}",
        screens[1]
    );
    assert!(
        screens[2]
            .lines()
            .next()
            .unwrap()
            .contains("LLP 0002.000  Gamma"),
        "{}",
        screens[2]
    );
}

#[test]
fn the_outline_goes_to_a_heading() {
    let screens = run(
        "outline",
        &[
            "key",
            "o",
            "key",
            "ArrowDown",
            "key",
            "ArrowDown",
            "key",
            "Enter",
            "print",
        ],
    );
    let s = &screens[0];
    let second = s.lines().nth(1).unwrap_or_default();
    assert!(
        second.contains("Decisions"),
        "the heading is at the top:\n{s}"
    );
}

#[test]
fn a_click_on_a_cross_reference_opens_it_and_b_comes_back() {
    let id = link("click", "LLP 0002");
    let screens = run(
        "click",
        &[
            "tap", &id, "wait", "200", "print", "key", "b", "wait", "200", "print",
        ],
    );
    assert!(
        screens[0]
            .lines()
            .next()
            .unwrap()
            .contains("LLP 0002  Beta"),
        "{}",
        screens[0]
    );
    assert!(screens[0].contains("b back"), "{}", screens[0]);
    assert!(
        screens[1]
            .lines()
            .next()
            .unwrap()
            .contains("LLP 0001  Alpha"),
        "{}",
        screens[1]
    );
}

#[test]
fn a_click_on_a_web_link_goes_to_the_system() {
    let id = link("web", "the site");
    let (screens, said) = run_err("web", &["tap", &id, "print"]);
    assert!(said.contains("open https://example.com/llp"), "{said}");
    assert!(
        screens[0].lines().next().unwrap().contains("Alpha"),
        "{}",
        screens[0]
    );
}

#[test]
fn j_k_and_g_shift_g_move_through_the_page() {
    let screens = run(
        "keys",
        &[
            "key", "j", "key", "j", "print", "key", "k", "print", "key", "G", "print", "key", "g",
            "print",
        ],
    );
    // The page's first row is a blank one above the title.
    let second = |s: &String| s.lines().nth(2).unwrap_or_default().to_string();
    let top = second(&screens[3]);
    assert!(
        top.contains("LLP 0001: Alpha"),
        "g is the top: {}",
        screens[3]
    );
    assert_ne!(second(&screens[0]), top, "j scrolled: {}", screens[0]);
    assert_ne!(
        second(&screens[1]),
        second(&screens[0]),
        "k came back a row"
    );
    assert!(
        screens[2].contains("Seven."),
        "G is the end: {}",
        screens[2]
    );
}

#[test]
fn shift_n_walks_the_matches_back() {
    let screens = run(
        "shiftn",
        &[
            "key", "/", "type", "find", "needle", "key", "Enter", "key", "N", "print",
        ],
    );
    assert!(screens[0].contains("2/2 “needle”"), "{}", screens[0]);
}
