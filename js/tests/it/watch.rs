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

/// LLP 1069.010.000 D5: under `doc.keep` an answer lists what the host keeps,
/// hears the list change, and forgets one; forgetting it again is `ENOENT`.
#[test]
fn an_answer_lists_and_forgets_what_the_app_keeps() {
    use exact_data::documents::kept;
    let root = Root::new();
    std::fs::create_dir_all(&root.0).unwrap();
    std::fs::write(root.0.join("kept.md"), "kept").unwrap();
    kept::open(9131, "test.js.kept", &root.0.join("kept.json"));
    let doc = kept::keep(9131, &root.0.join("kept.md"), "", "readwrite").unwrap();
    let mut module = root.module();
    module.set_document_owner(9131);
    module.activate().unwrap();
    let mut store = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(
        call(&mut module, &mut store, "kept", &doc),
        format!("{doc} file readwrite | 0 | ENOENT")
    );
    assert_eq!(store.take_topics(), [kept::LIST_TOPIC]);
    kept::end(9131);
    exact_data::documents::forget(9131);
}

/// LLP 1069.010.000 D4: the app's own `atomicWriteFile` of a kept file from
/// TypeScript replaces it with a new file, and the entry is carried to it,
/// so the next launch reopens what the app saved.
#[cfg(unix)]
#[test]
fn an_atomic_save_from_typescript_keeps_the_document() {
    use exact_data::documents::kept;
    use std::os::unix::fs::MetadataExt;
    let root = Root::new();
    std::fs::create_dir_all(&root.0).unwrap();
    let file = root.0.join("saved.md");
    std::fs::write(&file, "before").unwrap();
    let before = std::fs::metadata(&file).unwrap().ino();
    let set = root.0.join("kept.json");
    kept::open(9132, "test.js.kept-save", &set);
    let doc = kept::keep(9132, &file, "", "readwrite").unwrap();
    let mut module = root.module();
    module.set_document_owner(9132);
    module.activate().unwrap();
    let mut store = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(call(&mut module, &mut store, "kept-save", &doc), doc);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "saved");
    let after = std::fs::metadata(&file).unwrap().ino();
    assert_ne!(before, after, "an atomic write is a new file");
    let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&set).unwrap()).unwrap();
    assert_eq!(written["documents"][0]["inode"], after);
    kept::end(9132);
    exact_data::documents::forget(9132);
}
