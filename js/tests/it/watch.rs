//! Document watches and expected writes from TypeScript (LLP 1069.010.000
//! D1, D2a), through the native storage provider.
#![cfg(exact_js_engine)]
use super::storage::{call, Root, GRANTS};
use exact_runner::{DataSource, Store};

#[test]
fn an_answer_watches_a_chosen_document_and_an_expected_write_refuses_a_change() {
    let root = Root::new();
    let folder = root.0.join("chosen");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("drawing.json"), "one").unwrap();
    let doc =
        exact_data::documents::open_route(folder.join("drawing.json").to_str().unwrap(), 9130)
            .unwrap();
    let mut module = root.module();
    module.set_document_owner(9130);
    module.activate().unwrap();
    let mut store = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(
        call(&mut module, &mut store, "doc-watch", &doc),
        "Unavailable ECHANGED then written"
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("drawing.json")).unwrap(),
        "mine"
    );
    let folder_doc = doc.rsplit_once('/').unwrap().0;
    let mut topics = store.take_topics();
    topics.sort();
    assert_eq!(
        topics,
        [
            exact_data::documents::watch_topic(folder_doc, true),
            exact_data::documents::watch_topic(&doc, false),
        ]
    );
    exact_data::documents::forget(9130);
}
