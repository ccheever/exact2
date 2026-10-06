//! The kernel-free render, the differential check's fast lane (LLP 1048.004
//! D6): a document written from its runner's instance tree is the one a
//! kernel's nodes give, byte for byte — the fixture's pages, and every
//! corpus and conformance plan's first page.

use super::{plan, warm_transport, Blog, Post, SITE};
use exact_plan::Value;
use exact_render::{render_with_at, Ids, Projection, Rendered};
use exact_runner::{Answer, DataError, DataSource, Request, Store};
use std::path::Path;
use std::time::Duration;

fn same(kernel: &Rendered, direct: &Rendered, what: &str) {
    assert_eq!(
        kernel.document.root, direct.document.root,
        "{what}: the root"
    );
    assert_eq!(kernel.head, direct.head, "{what}: the head");
    assert_eq!(
        kernel.checkpoint, direct.checkpoint,
        "{what}: the checkpoint"
    );
    assert_eq!(
        kernel.document.keyframes, direct.document.keyframes,
        "{what}: the keyframes"
    );
    assert_eq!(
        kernel.document.scroll_document, direct.document.scroll_document,
        "{what}: the scroller"
    );
    assert_eq!(kernel.activate, direct.activate, "{what}: the activation");
    assert_eq!(kernel.settled, direct.settled, "{what}: how it settled");
}

#[test]
fn the_fixture_s_pages_are_the_same_without_a_kernel() {
    warm_transport();
    let plan = plan();
    for (post, deadline) in [
        (Post::Soon, Duration::from_secs(5)),
        (Post::Never, Duration::from_millis(150)),
        (Post::Storage, Duration::from_secs(5)),
        (Post::Elsewhere, Duration::from_secs(5)),
    ] {
        let data = || Blog::new(post);
        let render = |projection| {
            render_with_at(
                &plan,
                &data,
                Default::default(),
                "/post/7",
                &SITE,
                deadline,
                Ids::Any,
                projection,
                1_700_000_000_000.0,
            )
            .unwrap()
        };
        let (kernel, direct) = (render(Projection::Kernel), render(Projection::Direct));
        same(&kernel, &direct, "/post/7");
    }
}

/// A source that answers every call later, from an origin no grant names:
/// each resource keeps its placeholder, as a render refused by its
/// environment shows it.
#[derive(Default)]
struct Nowhere;

impl DataSource for Nowhere {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }

    fn answer(&mut self, _: &mut Store, _: &str, _: &[Value]) -> Result<Answer, DataError> {
        Ok(Answer::Later(Request::get("https://nowhere.invalid/")))
    }
}

#[test]
fn every_corpus_and_conformance_plan_is_the_same_without_a_kernel() {
    warm_transport();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files: Vec<_> = ["contract/corpus", "host/web-js/conformance"]
        .iter()
        .flat_map(|dir| std::fs::read_dir(root.join(dir)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "contract"))
        .collect();
    files.sort();
    let (mut compared, mut skipped) = (0, Vec::new());
    for file in &files {
        let Ok(plan) = contract::compile(&std::fs::read_to_string(file).unwrap()) else {
            continue;
        };
        let render = |projection| {
            render_with_at(
                &plan,
                &|| Nowhere,
                Default::default(),
                "/",
                &SITE,
                Duration::from_secs(2),
                Ids::Any,
                projection,
                1_700_000_000_000.0,
            )
        };
        let what = file.display().to_string();
        let Ok(kernel) = render(Projection::Kernel) else {
            skipped.push(format!("{what}: no kernel render"));
            continue;
        };
        match render(Projection::Direct) {
            Ok(direct) => {
                same(&kernel, &direct, &what);
                compared += 1;
            }
            Err(why) => skipped.push(format!("{what}: {why}")),
        }
    }
    // Every plan a kernel renders is written without one too (LLP
    // 1048.004 stage 2: virtualized lists and `id` references included),
    // except one whose initializer reads a resource through a prop, which
    // `projects_as_booted` sends to the kernel (budget.contract, 976bef093).
    assert!(
        skipped.iter().all(|why| why.ends_with("no kernel render")
            || why.ends_with("a slot's initializer reads a resource"))
            && compared > 0,
        "{compared} of {} plans compared; skipped: {skipped:#?}",
        files.len()
    );
}
