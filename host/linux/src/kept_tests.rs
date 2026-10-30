use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What GTK's `GtkRecentManager` writes, with another app's entry.
const GTK: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0"
      xmlns:bookmark="http://www.freedesktop.org/standards/desktop-bookmarks"
      xmlns:mime="http://www.freedesktop.org/standards/shared-mime-info"
>
  <bookmark href="file:///SHARED" added="2026-10-01T09:00:00.000000Z" modified="2026-10-01T09:00:00.000000Z" visited="2026-10-01T09:00:00.000000Z">
    <info>
      <metadata owner="http://freedesktop.org">
        <mime:mime-type type="text/markdown"/>
        <bookmark:applications>
          <bookmark:application name="org.gnome.TextEditor" exec="&apos;gnome-text-editor %u&apos;" modified="2026-10-01T09:00:00.000000Z" count="2"/>
        </bookmark:applications>
      </metadata>
    </info>
  </bookmark>
</xbel>
"#;

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.to_string())
    }
}

/// D6: a desktop session keeps; a run with no display, or no desktop, does not.
#[test]
fn only_a_desktop_session_keeps() {
    assert!(desktop_session(env(&[("XDG_SESSION_TYPE", "wayland")])));
    assert!(desktop_session(env(&[("XDG_SESSION_TYPE", "x11")])));
    assert!(desktop_session(env(&[("XDG_CURRENT_DESKTOP", "GNOME")])));
    assert!(!desktop_session(env(&[("XDG_SESSION_TYPE", "tty")])));
    assert!(!desktop_session(env(&[])));
    for headless in [
        ("EXACT_AGENT", "1"),
        ("EXACT_SMOKE", "1"),
        ("EXACT_SHOT", "/tmp/a.png"),
        ("EXACT_DISPLAY", "headless"),
    ] {
        assert!(!desktop_session(env(&[
            ("XDG_SESSION_TYPE", "wayland"),
            headless
        ])));
    }
}

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("exact-linux-kept-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// The store keeps every other app's entries as written: a new document is
/// a new entry, one another app already lists gains this app's mark, and
/// removing it takes off this app's mark alone, or the entry it alone marks.
/// Unix only: an entry's identity is its device and inode.
#[cfg(unix)]
#[test]
fn the_store_is_marked_and_unmarked_for_the_app() {
    let root = scratch("store");
    let (shared, mine) = (root.join("shared.md"), root.join("mine.md"));
    std::fs::write(&shared, "s").unwrap();
    std::fs::write(&mine, "m").unwrap();
    let file = root.join(STORE);
    let href = url::Url::from_file_path(&shared).unwrap();
    let gtk = GTK.replace("file:///SHARED", href.as_str());
    std::fs::write(&file, &gtk).unwrap();
    let recents = Recents::at(file.clone(), "com.exact.test", Path::new("/opt/exact/app"));
    assert!(recents.held().is_empty());
    recents.note(&mine).unwrap();
    recents.note(&shared).unwrap();
    recents.note(&mine).unwrap();
    let mut held = recents.held();
    held.sort();
    assert_eq!(held, [mine.clone(), shared.clone()]);
    let text = std::fs::read_to_string(&file).unwrap();
    let doc = roxmltree::Document::parse(&text).unwrap();
    assert_eq!(bookmarks(&doc).count(), 2);
    assert!(text.contains(r#"name="org.gnome.TextEditor" exec="&apos;gnome-text-editor %u&apos;""#));
    assert_eq!(recents.exec, r#"''\''/opt/exact/app'\'' %u'"#);
    assert!(text.contains(&format!("exec=\"{}\"", escape(&recents.exec))));
    let ours = bookmarks(&doc)
        .find(|b| path_of(*b).as_deref() == Some(&*mine))
        .unwrap();
    assert_eq!(
        mark(&ours, "com.exact.test").unwrap().attribute("count"),
        Some("2")
    );
    let identity = |p: &Path| Identity::of(p).unwrap().0;
    recents.remove(identity(&shared), &shared).unwrap();
    assert_eq!(recents.held(), std::slice::from_ref(&mine));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("org.gnome.TextEditor"),
        "the other app's entry stays"
    );
    recents.remove(identity(&mine), &mine).unwrap();
    assert!(recents.held().is_empty());
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        bookmarks(&roxmltree::Document::parse(&text).unwrap()).count(),
        1
    );
    // A store that does not parse is the person's: left as it is.
    std::fs::write(&file, "<xbel").unwrap();
    assert!(recents.note(&mine).is_err());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "<xbel");
    // None yet: one is written.
    std::fs::remove_file(&file).unwrap();
    recents.note(&mine).unwrap();
    assert_eq!(recents.held(), [mine]);
    std::fs::remove_dir_all(root).unwrap();
}

/// What other writers may have written still takes this app's entries as
/// a well-formed store: a root written empty, namespaces declared below the
/// root or by other prefixes. The store stays private (`0600`), the command
/// is quoted as GLib unquotes it, at most [`MOST`] entries carry the app's
/// mark, and a forgotten document's mark goes even when its file is gone.
#[cfg(unix)]
#[test]
fn the_store_stays_well_formed_bounded_and_private() {
    use std::os::unix::fs::PermissionsExt;
    let root = scratch("forms");
    let file = root.join(STORE);
    let doc = root.join("it's here.md");
    std::fs::write(&doc, "d").unwrap();
    let recents = Recents::at(
        file.clone(),
        "com.exact.test",
        Path::new("/opt/my app/it's"),
    );
    assert_eq!(recents.exec, r#"''\''/opt/my app/it'\''\'\'''\''s'\'' %u'"#);
    let other = |prefix: &str| {
        format!(
            r#"<xbel version="1.0"><bookmark href="file:///elsewhere" added="a" modified="a" visited="a"><info><metadata owner="http://freedesktop.org" xmlns:{prefix}="{BOOKMARK_NS}"><{prefix}:applications><{prefix}:application name="other" exec="x" modified="a" count="1"/></{prefix}:applications></metadata></info></bookmark></xbel>"#
        )
    };
    let href = url::Url::from_file_path(&doc).unwrap();
    for written in [
        r#"<xbel version="1.0"/>"#.to_owned(),
        other("bookmark"),
        other("bm"),
    ] {
        std::fs::write(&file, &written).unwrap();
        recents.note(&doc).unwrap();
        assert_eq!(recents.held(), std::slice::from_ref(&doc), "{written}");
        // Marked where another app marks it, under the prefix in scope there.
        std::fs::write(&file, written.replace("file:///elsewhere", href.as_str())).unwrap();
        recents.note(&doc).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert_eq!(recents.held(), std::slice::from_ref(&doc), "{text}");
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Bounded: the oldest of this app's entries lose its mark.
    std::fs::remove_file(&file).unwrap();
    let docs: Vec<PathBuf> = (0..MOST + 2)
        .map(|i| {
            let p = root.join(format!("{i}.md"));
            std::fs::write(&p, "x").unwrap();
            recents.note(&p).unwrap();
            p
        })
        .collect();
    let held = recents.held();
    assert_eq!(held.len(), MOST);
    assert!(!held.contains(&docs[0]) && !held.contains(&docs[1]));
    assert!(held.contains(&docs[MOST + 1]));
    // A forgotten document whose file is gone: its mark goes by its last path.
    let last = &docs[MOST + 1];
    let identity = Identity::of(last).unwrap().0;
    std::fs::remove_file(last).unwrap();
    recents.remove(identity, last).unwrap();
    assert!(!recents.held().contains(last));
    std::fs::remove_dir_all(root).unwrap();
}

/// Another writer's store, renamed into place while this app edits it, is
/// edited again rather than overwritten.
#[test]
fn an_edit_made_while_another_writer_replaced_the_store_is_made_again() {
    let root = scratch("race");
    let file = root.join(STORE);
    std::fs::write(&file, GTK.replace("/SHARED", "/tmp/a")).unwrap();
    let recents = Recents::at(file.clone(), "com.exact.test", Path::new("/opt/exact/app"));
    let tries = AtomicUsize::new(0);
    recents
        .edit(|text, doc| {
            if tries.fetch_add(1, Ordering::SeqCst) == 0 {
                // Another writer adds an entry and renames its store in.
                let theirs = GTK.replace("/SHARED", "/tmp/a").replace(
                    "</xbel>",
                    r#"  <bookmark href="file:///tmp/b" added="a" modified="a" visited="a"/>
</xbel>"#,
                );
                std::fs::write(root.join("theirs"), theirs).unwrap();
                std::fs::rename(root.join("theirs"), &file).unwrap();
            }
            Ok(append(
                text,
                &doc.root_element(),
                r#"<bookmark href="file:///tmp/c"/>"#.into(),
            ))
        })
        .unwrap();
    assert_eq!(tries.load(Ordering::SeqCst), 2);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("file:///tmp/b") && text.contains("file:///tmp/c"),
        "{text}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn times_are_written_as_glib_writes_them() {
    let at = std::time::UNIX_EPOCH + std::time::Duration::from_micros(1_761_480_000_123_456);
    assert_eq!(timestamp(at), "2025-10-26T12:00:00.123456Z");
    assert_eq!(
        timestamp(std::time::UNIX_EPOCH),
        "1970-01-01T00:00:00.000000Z"
    );
}

const PHASE: &str = "EXACT_LINUX_KEPT_PHASE";
const ROOT: &str = "EXACT_LINUX_KEPT_ROOT";
const APP: &str = "test.exact.kept";

/// D3–D6 in a desktop session (the session check passed by calling past it),
/// each launch its own process: a chosen folder and file reopen at the same
/// paths; one another program replaced is forgotten; `forgetDocument` takes
/// the app's mark off the store; the person clearing the store while the app
/// runs drops the rest, and after a relaunch the path refuses.
#[cfg(unix)]
#[test]
fn chosen_documents_outlast_the_launch_in_a_desktop_session() {
    let Some(phase) = std::env::var_os(PHASE) else {
        let root = scratch("launches");
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/a.md"), "a").unwrap();
        std::fs::write(root.join("file.md"), "f").unwrap();
        std::fs::write(root.join("other.md"), "o").unwrap();
        let launch = |phase: &str| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "kept::tests::chosen_documents_outlast_the_launch_in_a_desktop_session",
                ])
                .env(PHASE, phase)
                .env(ROOT, &root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "launch {phase}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        launch("choose");
        // Another program replaces a kept file at its name while the app is closed.
        std::fs::write(root.join("other.md.new"), "impostor").unwrap();
        std::fs::rename(root.join("other.md.new"), root.join("other.md")).unwrap();
        launch("reopen");
        launch("cleared");
        std::fs::remove_dir_all(root).unwrap();
        return;
    };
    let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
    let announced = Arc::new(AtomicUsize::new(0));
    let heard = announced.clone();
    let store = Recents::at(root.join(STORE), APP, Path::new("/opt/exact/app"));
    begin(
        0,
        APP,
        &root.join("kept-documents.json"),
        store,
        Arc::new(move || {
            heard.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let listed = || -> Vec<String> {
        kept::list(0)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["path"].as_str().unwrap().to_owned())
            .collect()
    };
    let docs = root.join("docs.txt");
    let read = |path: &str| exact_data::documents::resolve(path, 0).map(drop);
    match phase.to_str().unwrap() {
        "choose" => {
            let folder = chosen(0, &root.join("notes")).unwrap();
            let file = chosen(0, &root.join("file.md")).unwrap();
            let other = chosen(0, &root.join("other.md")).unwrap();
            assert_eq!(listed(), [&*other, &*file, &*folder]);
            assert_eq!(super::recents().unwrap().held().len(), 3);
            std::fs::write(&docs, [folder, file, other].join("\n")).unwrap();
        }
        "reopen" => {
            let text = std::fs::read_to_string(&docs).unwrap();
            let [folder, file, other] = text.lines().collect::<Vec<_>>()[..] else {
                panic!("{text}");
            };
            assert_eq!(listed(), [file, folder], "the replaced file is forgotten");
            read(&format!("{folder}/a.md")).unwrap();
            read(file).unwrap();
            assert!(read(other).is_err());
            // A new handle is numbered above every one an earlier launch gave.
            let fresh = exact_data::documents::mint(&root.join("other.md"), 0).unwrap();
            let n = |p: &str| p.split('/').nth(1).unwrap().parse::<u64>().unwrap();
            assert!(n(&fresh) > n(other), "{fresh} after {other}");
            kept::forget(0, file).unwrap();
            assert_eq!(listed(), [folder]);
            // Off the store; the replaced file's entry is the desktop's, and
            // names the replacement, which was never chosen here.
            let held = super::recents().unwrap().held();
            assert!(held.contains(&root.join("notes")) && !held.contains(&root.join("file.md")));
            // The person clears the desktop's recent history while the app runs.
            let before = announced.load(Ordering::SeqCst);
            std::fs::write(root.join(STORE), EMPTY).unwrap();
            recheck(0);
            assert!(listed().is_empty());
            assert!(announced.load(Ordering::SeqCst) > before);
            // Live access lasts while the app runs (D4).
            read(&format!("{folder}/a.md")).unwrap();
        }
        _ => {
            let text = std::fs::read_to_string(&docs).unwrap();
            let folder = text.lines().next().unwrap();
            assert!(listed().is_empty());
            let refused = exact_data::documents::resolve(&format!("{folder}/a.md"), 0).unwrap_err();
            assert_eq!(refused.kind(), std::io::ErrorKind::NotFound);
        }
    }
}
