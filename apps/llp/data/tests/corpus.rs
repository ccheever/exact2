//! The corpus this reader exists for is the one in this repository, so that
//! is what these check against. @ref LLP 1033

use llp_data::link_references;
use markdown_parse::{plain, Run};
use std::collections::BTreeSet;

fn llp() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llp")
}

#[test]
fn the_index_is_this_repositorys_corpus_in_number_order() {
    let entries = llp_data::read_index(&llp()).expect("llp/");
    assert!(entries.len() > 30, "found {} documents", entries.len());

    let root = entries
        .iter()
        .find(|e| e.number == "1000")
        .expect("the root explainer");
    assert_eq!(root.kind, "explainer");
    assert_eq!(root.depth, 0);
    assert!(
        root.title.starts_with("Exact2 Project Root"),
        "{}",
        root.title
    );
    assert_eq!(root.overlay, "foundation");

    // A dotted number is a sub-document, and sorts under its parent.
    let sub = entries.iter().position(|e| e.number == "1030.000").unwrap();
    let parent = entries.iter().position(|e| e.number == "1030").unwrap();
    assert_eq!(entries[sub].depth, 1);
    assert!(parent < sub, "1030.000 sorts after 1030");
    assert!(entries[sub..].iter().all(|e| e.number != "1029"));

    // The working set is the `current/` overlay, and it is declared.
    let current: BTreeSet<_> = entries
        .iter()
        .filter(|e| e.overlay == "current")
        .map(|e| e.path.file_name().unwrap().to_owned())
        .collect();
    let declared: BTreeSet<_> = std::fs::read_dir(llp().join("current"))
        .expect("llp/current/")
        .map(|e| e.expect("working-set entry").file_name())
        .collect();
    assert_eq!(current, declared, "the reader indexes every declared link");
    assert!(
        current.len() <= 15,
        "rules/RULES.md caps the working set at 15, found {}",
        current.len()
    );
}

#[test]
fn search_looks_through_every_documents_text() {
    let entries = llp_data::read_index(&llp()).expect("llp/");
    let kernel = entries.iter().find(|e| e.number == "1001").unwrap();
    assert!(kernel.matches("taffy"), "the kernel spec names Taffy");
    assert!(kernel.hits("taffy") > 1);
    assert!(!kernel.matches("a phrase that appears in no document at all"));
    // A number and a status match too.
    assert!(kernel.matches("1001"));
}

#[test]
fn a_reference_in_prose_becomes_a_link_and_an_unknown_one_does_not() {
    let runs = vec![Run::text("See LLP 1030 D7 and LLP 9999 for the rest.")];
    let linked = link_references(runs, &|number| {
        (number == "1030").then(|| "/llp/1030-delivery-unified.rfc.md".to_string())
    });
    assert_eq!(plain(&linked), "See LLP 1030 D7 and LLP 9999 for the rest.");
    let link: Vec<&Run> = linked.iter().filter(|r| !r.href.is_empty()).collect();
    assert_eq!(link.len(), 1);
    assert_eq!(link[0].text, "LLP 1030");
    assert_eq!(link[0].href, "/llp/1030-delivery-unified.rfc.md");
}

#[test]
fn a_dotted_reference_resolves_to_the_sub_document() {
    let linked = link_references(vec![Run::text("as LLP 1030.000 says")], &|number| {
        (number == "1030.000").then(|| "/llp/1030.000.rfc.md".to_string())
    });
    let link = linked.iter().find(|r| !r.href.is_empty()).unwrap();
    assert_eq!(link.text, "LLP 1030.000");
}

#[test]
fn code_and_existing_links_are_left_alone() {
    let code = Run {
        text: "LLP 1030".into(),
        code: true,
        ..Run::default()
    };
    let already = Run {
        text: "LLP 1030".into(),
        href: "https://example.com".into(),
        ..Run::default()
    };
    let linked = link_references(vec![code, already], &|_| Some("/somewhere".to_string()));
    assert!(linked[0].href.is_empty(), "a code span is code");
    assert_eq!(linked[1].href, "https://example.com");
}
