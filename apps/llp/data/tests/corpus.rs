//! The corpus this reader exists for is the one in this repository, so that
//! is what these check against. @ref LLP 1033

use llp_data::link_references;
use markdown_parse::{plain, Run};
use std::collections::BTreeSet;

fn llp() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../llp")
}

#[test]
fn simultaneous_page_and_index_requests_read_one_corpus_snapshot() {
    use exact_plan::Value;
    use exact_runner::{Answer, DataSource, Store};
    let root = std::env::temp_dir().join(format!("llp-shared-corpus-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("1000-one.explainer.md");
    std::fs::write(&path, "# First snapshot\n\nOriginal body.").unwrap();
    let mut data = llp_data::Llp::new();
    let mut store = Store::new("", []);
    let args = [Value::str(&root.to_string_lossy()), Value::str("")];
    let mut workers = Vec::new();
    for source in ["page", "index"] {
        let Answer::Later(request) = data.answer(&mut store, source, &args).unwrap() else {
            panic!("the corpus should load on a worker");
        };
        workers.push(data.continuation(request.continuation.unwrap()).unwrap());
    }
    let first = workers.remove(0)();
    data.parse(&mut store, "page", &args, first).unwrap();
    // The other waiter must receive the same snapshot even when it runs
    // after the first resource has consumed its answer.
    std::fs::write(&path, "# Replaced snapshot\n\nNew body.").unwrap();
    let second = workers.remove(0)();
    let Answer::Now(index) = data.parse(&mut store, "index", &args, second).unwrap() else {
        panic!("the corpus is ready");
    };
    assert!(format!("{index:?}").contains("First snapshot"));
    assert!(!format!("{index:?}").contains("Replaced snapshot"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corpus_text_keeps_document_boundaries_and_survives_its_index() {
    let root = std::env::temp_dir().join(format!("llp-text-corpus-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let first = "# LLP 0001: First\n\nİstanbul Σίσυφος 界 é needle NEEDLE.";
    let second = "# LLP 0002: Second\n\nOther document.";
    std::fs::write(root.join("0001-first.rfc.md"), first).unwrap();
    std::fs::write(root.join("0002-second.rfc.md"), second).unwrap();
    std::fs::write(root.join("0003-invalid.rfc.md"), [0xff]).unwrap();
    std::fs::write(root.join("0004-empty.rfc.md"), "").unwrap();
    let entries = llp_data::read_index(&root).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(&*entries[0].body, first);
    assert_eq!(&*entries[1].body, second);
    assert_eq!(&*entries[2].body, "");
    assert_eq!(entries[0].search("needle"), Some(2));
    assert_eq!(entries[1].search("needle"), None);
    assert_eq!(entries[0].search("i\u{307}stanbul"), Some(1));
    assert!(!format!("{:?}", entries[1].body).contains("First"));
    let kept = entries[0].clone();
    let reread = llp_data::read_index(&root).unwrap();
    assert_eq!(kept, reread[0]);
    drop(entries);
    drop(reread);
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(&*kept.body, first);
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
    assert!(
        kernel.search("taffy").is_some_and(|hits| hits > 1),
        "the kernel spec names Taffy more than once"
    );
    assert_eq!(
        kernel.search("a phrase that appears in no document at all"),
        None
    );
    // A number and a status match too.
    assert!(kernel.search("1001").is_some());
}

#[test]
fn search_counts_body_hits_and_keeps_metadata_only_matches() {
    let root = std::env::temp_dir().join(format!("llp-search-corpus-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("0007-zeta.rfc.md"),
        "# ΖΕΤΑ\n\n**Status:** Reviewed\n\nΟΣ ΟΣΑ Σ ΣΣ İ AAaaa",
    )
    .unwrap();
    let entries = llp_data::read_index(&root).unwrap();
    let entry = &entries[0];
    for (query, expected) in [
        ("", Some(0)),
        ("0007", Some(0)),
        ("rfc", Some(0)),
        ("reviewed", Some(1)),
        ("ζετα", Some(1)),
        ("ος", Some(1)),
        ("οσ", Some(1)),
        ("i\u{307}", Some(1)),
        ("aa", Some(2)),
        ("absent", None),
    ] {
        assert_eq!(entry.search(query), expected, "query {query:?}");
    }
    std::fs::remove_dir_all(root).unwrap();
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
