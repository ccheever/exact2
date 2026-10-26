//! `storage.fs.copyFile` from a chosen document into an app file (LLP
//! 1069.010 D1, ruled 2026-10-26), through the native storage provider.
#![cfg(exact_js_engine)]
use super::storage::{call, Root, GRANTS};
use exact_runner::{DataSource, Store};

/// Past the 16 MiB a `readFile` answers within, since the bytes never reach
/// JavaScript; refused into an app file no `fs.write` covers, and between
/// documents either way.
#[test]
fn a_chosen_file_is_copied_into_an_app_file_by_the_host() {
    let root = Root::new();
    let folder = root.0.join("chosen");
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder.join("big.bin");
    let size = exact_data::storage::MAX_BYTES as u64 + 1;
    std::fs::File::create(&file).unwrap().set_len(size).unwrap();
    let doc = exact_data::documents::mint(&file, 9140).unwrap();
    let mut module = root.module();
    module.set_document_owner(9140);
    module.activate().unwrap();
    let mut store = Store::new(GRANTS, Vec::<(String, Vec<u8>)>::new());
    assert_eq!(
        call(&mut module, &mut store, "doc-copy", &doc),
        format!("copied {size} Unavailable denied Unavailable failed Unavailable failed Unavailable failed")
    );
    assert_eq!(
        std::fs::metadata(root.0.join("data/copied")).unwrap().len(),
        size
    );
    assert!(!root.0.join("cache/ungranted").exists());
    assert!(!folder.join("copy").exists());
    assert!(file.exists());
    exact_data::documents::forget(9140);
}
