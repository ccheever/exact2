#[cfg(unix)]
use super::super::{mint, relaunch, resolve, Resolved};
use super::*;

/// A scratch folder and a kept set's file for one test.
#[cfg(unix)]
fn scratch(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("exact-kept-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("notes")).unwrap();
    std::fs::write(root.join("notes/a.md"), "a").unwrap();
    std::fs::write(root.join("notes/b.md"), "b").unwrap();
    let canonical = std::fs::canonicalize(&root).unwrap();
    (canonical.clone(), canonical.join("kept.json"))
}

#[cfg(unix)]
fn paths(owner: u64) -> Vec<String> {
    list(owner)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap().to_owned())
        .collect()
}

/// What a launch's host does: open the set, find each object where its
/// path says, and say the platform's list holds them all.
#[cfg(unix)]
fn launch(owner: u64, app: &str, file: &Path) {
    open(owner, app, file);
    let found: Vec<_> = to_restore(owner)
        .into_iter()
        .map(|r| (r.n, r.path, None))
        .collect();
    restore(owner, &found);
}

#[cfg(unix)]
fn reads(path: &str, owner: u64) -> bool {
    matches!(resolve(path, owner), Ok(Resolved::Real { .. }))
}

#[test]
#[cfg(unix)]
fn a_kept_document_resolves_after_a_relaunch_and_its_number_is_never_given_again() {
    let (root, file) = scratch("relaunch");
    let app = "test.kept.relaunch";
    launch(7501, app, &file);
    // A route's file arrives beneath its folder's handle; only the file is kept.
    let folder = mint(&root.join("notes"), 7501).unwrap();
    let doc = keep(7501, &root.join("notes/a.md"), "bm", "readwrite").unwrap();
    assert!(doc.ends_with("/a.md") && !doc.starts_with(&folder), "{doc}");
    let chosen = keep(7501, &root.join("notes"), "bm2", "readwrite").unwrap();
    assert_eq!(chosen, folder, "a picker's own handle is the one kept");
    assert_eq!(
        paths(7501),
        [chosen.clone(), doc.clone()],
        "most recent first"
    );
    assert_eq!(
        keep(7501, &root.join("notes/a.md"), "bm3", "readwrite").unwrap(),
        doc
    );
    assert_eq!(
        paths(7501),
        [doc.clone(), chosen.clone()],
        "chosen again: the same entry, first"
    );
    let written = list(7501).unwrap()[0].clone();
    assert_eq!(
        (
            written["name"].as_str(),
            written["kind"].as_str(),
            written["mode"].as_str()
        ),
        (Some("a.md"), Some("file"), Some("readwrite"))
    );

    relaunch(app);
    assert!(
        !reads(&doc, 7502),
        "nothing resolves before the set is opened"
    );
    launch(7502, app, &file);
    assert_eq!(paths(7502), [doc.clone(), chosen.clone()]);
    assert!(reads(&doc, 7502) && reads(&format!("{chosen}/b.md"), 7502));
    assert!(
        resolve(&format!("{doc}/../b.md"), 7502).is_err()
            && !reads(&format!("{folder}/a.md").replace(&folder, &doc), 7502),
        "a kept file reaches nothing beside it"
    );
    // Another window of the app reads it too; another app does not.
    open(7503, app, &file);
    assert!(reads(&doc, 7503));
    assert!(!reads(&doc, 7504));
    // A fresh mint is above every number the app gave before.
    let fresh = mint(&root.join("notes/b.md"), 7502).unwrap();
    let numbers = |p: &str| super::super::number(p).unwrap();
    assert!(
        numbers(&fresh) > numbers(&doc).max(numbers(&chosen)),
        "{fresh}"
    );
    // The session's own handle ends with it; the kept ones do not.
    end(7502);
    super::super::forget(7502);
    assert!(reads(&doc, 7503) && !reads(&fresh, 7503));

    relaunch(app);
    launch(7505, app, &file);
    let again = mint(&root.join("notes/b.md"), 7505).unwrap();
    assert!(
        numbers(&again) > numbers(&fresh),
        "a session's number is not given again either"
    );
    relaunch(app);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn a_replacement_at_the_path_is_not_the_document_and_a_move_is() {
    let (root, file) = scratch("identity");
    let app = "test.kept.identity";
    launch(7511, app, &file);
    let a = keep(7511, &root.join("notes/a.md"), "", "readwrite").unwrap();
    let b = keep(7511, &root.join("notes/b.md"), "", "readwrite").unwrap();
    relaunch(app);
    // Another program replaces a.md at its name; b.md moves.
    std::fs::rename(root.join("notes/a.md"), root.join("notes/a.md~")).unwrap();
    std::fs::write(root.join("notes/a.md"), "someone else's").unwrap();
    std::fs::rename(root.join("notes/b.md"), root.join("b-moved.md")).unwrap();
    open(7512, app, &file);
    let found: Vec<_> = to_restore(7512)
        .into_iter()
        .map(|r| {
            let path = if r.name == "b.md" {
                root.join("b-moved.md")
            } else {
                r.path
            };
            (r.n, path, Some("fresh".to_owned()))
        })
        .collect();
    restore(7512, &found);
    assert_eq!(
        paths(7512),
        std::slice::from_ref(&b),
        "the replacement does not inherit"
    );
    assert!(!reads(&a, 7512));
    let Ok(Resolved::Real { path, .. }) = resolve(&b, 7512) else {
        panic!()
    };
    assert_eq!(
        path, "b-moved.md",
        "the name at the choice names the object where it is now"
    );
    assert!(to_restore(7512).is_empty(), "restored once a launch");
    relaunch(app);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn the_platforms_list_and_the_apps_forget_drop_an_entry_and_its_own_save_keeps_it() {
    let (root, file) = scratch("forget");
    let app = "test.kept.forget";
    launch(7521, app, &file);
    let a = keep(7521, &root.join("notes/a.md"), "", "readwrite").unwrap();
    let b = keep(7521, &root.join("notes/b.md"), "", "readwrite").unwrap();
    let heard = Arc::new(Mutex::new(0));
    let count = heard.clone();
    listen(7521, Some(Arc::new(move || *count.lock().unwrap() += 1)));
    held(7521, &[root.join("notes/b.md"), root.join("nowhere")]);
    assert_eq!(
        paths(7521),
        std::slice::from_ref(&b),
        "a cleared or fallen-off entry goes"
    );
    assert_eq!(*heard.lock().unwrap(), 1, "the list's watchers hear it");
    assert!(reads(&a, 7521), "a session that has it keeps live access");
    assert!(forget(7521, &a).is_err() && forget(7521, "doc:/x").is_err());

    // The app's own atomic save through a route's folder handle: a new file
    // at the name, carried.
    let folder = mint(&root.join("notes"), 7521).unwrap();
    let replaced = before_write(7521, &format!("{folder}/b.md")).unwrap();
    assert!(
        before_write(7521, &format!("{folder}/a.md")).is_none(),
        "a.md is no longer kept"
    );
    std::fs::write(root.join("notes/.b.md.new"), "saved").unwrap();
    std::fs::rename(root.join("notes/.b.md.new"), root.join("notes/b.md")).unwrap();
    carry(7521, replaced);
    relaunch(app);
    launch(7522, app, &file);
    assert_eq!(
        paths(7522),
        std::slice::from_ref(&b),
        "carried to the file the app wrote"
    );

    let forgotten = Arc::new(Mutex::new(Vec::new()));
    let into = forgotten.clone();
    on_forgotten(Arc::new(move |identity, _| {
        into.lock().unwrap().push(identity)
    }));
    forget(7522, &b).unwrap();
    assert!(paths(7522).is_empty());
    let (identity, _) = Identity::of(&root.join("notes/b.md")).unwrap();
    assert!(
        forgotten.lock().unwrap().contains(&identity),
        "the host takes it off its list"
    );
    relaunch(app);
    launch(7523, app, &file);
    assert!(
        paths(7523).is_empty() && !reads(&b, 7523),
        "forgotten for good"
    );
    relaunch(app);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_session_of_an_app_that_keeps_nothing_lists_nothing() {
    assert!(list(7531).is_none());
    assert!(keep(7531, Path::new("/"), "", "readwrite").is_none());
}

#[test]
#[cfg(unix)]
fn what_the_list_no_longer_holds_is_never_restored_and_a_renamed_one_is_chosen_again_by_its_path() {
    let (root, file) = scratch("held");
    let app = "test.kept.held";
    launch(7541, app, &file);
    let a = keep(7541, &root.join("notes/a.md"), "", "readwrite").unwrap();
    let b = keep(7541, &root.join("notes/b.md"), "", "readwrite").unwrap();
    relaunch(app);
    // Cleared while the app was closed: the host says so before it restores.
    std::fs::rename(root.join("notes/b.md"), root.join("notes/renamed.md")).unwrap();
    open(7542, app, &file);
    let records = to_restore(7542);
    held(7542, &[root.join("notes/renamed.md")]);
    let found: Vec<_> = records
        .iter()
        .map(|r| {
            (
                r.n,
                if r.name == "b.md" {
                    root.join("notes/renamed.md")
                } else {
                    r.path.clone()
                },
                None,
            )
        })
        .collect();
    restore(7542, &found);
    assert!(
        !reads(&a, 7542),
        "a cleared entry does not resolve this launch either"
    );
    assert_eq!(paths(7542), std::slice::from_ref(&b));
    // Chosen again at its new name, it is the path it was kept by.
    assert_eq!(mint(&root.join("notes/renamed.md"), 7542).unwrap(), b);
    assert_eq!(
        keep(7542, &root.join("notes/renamed.md"), "", "readwrite").unwrap(),
        b
    );
    relaunch(app);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn a_set_that_does_not_read_keeps_nothing_and_is_left_as_it_is() {
    let (root, file) = scratch("corrupt");
    std::fs::write(&file, "not a kept set").unwrap();
    open(7551, "test.kept.corrupt", &file);
    assert!(list(7551).is_none());
    assert!(keep(7551, &root.join("notes/a.md"), "", "readwrite").is_none());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "not a kept set");
    let _ = std::fs::remove_dir_all(root);
}

/// Renamed while the app runs and chosen again, a kept document is the
/// entry it was, now where it is; a second process of the app keeps nothing.
#[test]
#[cfg(unix)]
fn a_document_renamed_this_launch_and_chosen_again_moves_its_entry() {
    let (root, file) = scratch("rechosen");
    let app = "test.kept.rechosen";
    launch(7561, app, &file);
    let a = keep(7561, &root.join("notes/a.md"), "", "readwrite").unwrap();
    std::fs::rename(root.join("notes/a.md"), root.join("notes/c.md")).unwrap();
    assert_eq!(
        keep(7561, &root.join("notes/c.md"), "", "readwrite").unwrap(),
        a
    );
    let Ok(Resolved::Real { path, .. }) = resolve(&a, 7561) else {
        panic!()
    };
    assert_eq!(path, "c.md");
    // The lock file is the first process's; another holding it keeps nothing.
    relaunch(app);
    let other = std::fs::OpenOptions::new()
        .write(true)
        .open(file.with_extension("lock"))
        .unwrap();
    other.try_lock().unwrap();
    open(7562, app, &file);
    assert!(list(7562).is_none());
    drop(other);
    let _ = std::fs::remove_dir_all(root);
}

/// Another object at a kept file's name, chosen, is a document of its own;
/// a save beneath a chosen folder is carried through the folder it is in.
#[test]
#[cfg(unix)]
fn a_replacement_chosen_is_its_own_document_and_a_save_in_a_subfolder_is_carried() {
    let (root, file) = scratch("own");
    let app = "test.kept.own";
    launch(7571, app, &file);
    let a = keep(7571, &root.join("notes/a.md"), "", "readwrite").unwrap();
    std::fs::rename(root.join("notes/a.md"), root.join("notes/a.md~")).unwrap();
    std::fs::write(root.join("notes/a.md"), "another").unwrap();
    let b = keep(7571, &root.join("notes/a.md"), "", "readwrite").unwrap();
    assert_ne!(a, b);
    std::fs::create_dir_all(root.join("notes/deep")).unwrap();
    std::fs::write(root.join("notes/deep/c.md"), "c").unwrap();
    let c = keep(7571, &root.join("notes/deep/c.md"), "", "readwrite").unwrap();
    let folder = mint(&root.join("notes"), 7571).unwrap();
    let replaced = before_write(7571, &format!("{folder}/deep/c.md")).unwrap();
    std::fs::rename(root.join("notes/deep"), root.join("notes/moved")).unwrap();
    std::fs::write(root.join("notes/moved/.c.new"), "saved").unwrap();
    std::fs::rename(
        root.join("notes/moved/.c.new"),
        root.join("notes/moved/c.md"),
    )
    .unwrap();
    carry(7571, replaced);
    relaunch(app);
    open(7572, app, &file);
    let found: Vec<_> = to_restore(7572)
        .into_iter()
        .map(|r| {
            let at = if r.name == "c.md" {
                root.join("notes/moved/c.md")
            } else {
                r.path
            };
            (r.n, at, None)
        })
        .collect();
    restore(7572, &found);
    assert!(
        paths(7572).contains(&c),
        "the save in a moved subfolder is carried"
    );
    relaunch(app);
    let _ = std::fs::remove_dir_all(root);
}
