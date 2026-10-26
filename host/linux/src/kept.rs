//! Documents kept across launches on Linux (@ref LLP 1069.010.000 D3–D6).
//!
//! An app granted `doc.keep` keeps what the person chose in a desktop
//! session, and only there: the list the person can clear from outside the
//! app is the freedesktop recent-files store, `recently-used.xbel` in
//! `$XDG_DATA_HOME` (the [Desktop Bookmark
//! Specification](https://www.freedesktop.org/wiki/Specifications/desktop-bookmark-spec/)),
//! each entry marked with the app's id as its `bookmark:application`. A
//! headless run, the DRM kiosk and a drive keep nothing (D6).
//!
//! The kept set is the library's (`exact_data::documents::kept`), in
//! `kept-documents.json` beside the app's `app:/` roots. On Linux the
//! path is the handle: an entry is restored only where the object recorded
//! (its device and inode) still is at the path it was last found at, so a
//! moved or replaced document is forgotten, never followed (D4).

use exact_data::documents::kept::{self, Identity};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The store's name in `$XDG_DATA_HOME`.
const STORE: &str = "recently-used.xbel";
const BOOKMARK_NS: &str = "http://www.freedesktop.org/standards/desktop-bookmarks";
const MIME_NS: &str = "http://www.freedesktop.org/standards/shared-mime-info";

/// The recent-files store this process notes its app's documents in, once
/// a session of an app that keeps documents has activated.
static RECENTS: Mutex<Option<Arc<Recents>>> = Mutex::new(None);

/// Whether this run is in a desktop session (D6), from its environment: a
/// display server's session (`XDG_SESSION_TYPE` `x11` or `wayland`, or a
/// desktop named by `XDG_CURRENT_DESKTOP`), and not a run without a
/// display (a drive, a smoke run, a screenshot, `EXACT_DISPLAY=headless`).
pub(crate) fn desktop_session(var: impl Fn(&str) -> Option<String>) -> bool {
    let set = |name: &str| var(name).is_some_and(|v| !v.is_empty());
    let headless = var("EXACT_AGENT").as_deref() == Some("1")
        || var("EXACT_SMOKE").as_deref() == Some("1")
        || set("EXACT_SHOT")
        || var("EXACT_DISPLAY").as_deref() == Some("headless");
    !headless
        && (matches!(var("XDG_SESSION_TYPE").as_deref(), Some("x11" | "wayland"))
            || set("XDG_CURRENT_DESKTOP"))
}

/// Begin keeping for `owner`, a session of the app `runner` runs, as its
/// data activates and before anything can read a kept document: read the
/// kept set, drop what the recent-files store no longer holds, and restore
/// each entry whose object is still where it was last found. Nothing for
/// an app without `doc.keep` or outside a desktop session.
pub(crate) fn activate<D: exact_runner::DataSource>(owner: u64, runner: &exact_runner::Runner<D>) {
    let desktop = cfg!(target_os = "linux") && desktop_session(|v| std::env::var(v).ok());
    let data = runner.data_ref();
    if !desktop || !keeps(data.grants()) {
        return;
    }
    let (Some(recents), Some(set)) = (Recents::of(data.app_id()), set_file(data.app_id())) else {
        return;
    };
    let native = runner.native_slot();
    begin(
        owner,
        data.app_id(),
        &set,
        recents,
        Arc::new(move || native.changed(kept::LIST_TOPIC)),
    );
}

fn keeps(grants: &str) -> bool {
    grants.lines().any(|line| line.trim() == "doc.keep")
}

/// Where `app_id` keeps its set: beside its `app:/` roots.
fn set_file(app_id: &str) -> Option<PathBuf> {
    let ([data, ..], _) = crate::picker::app_dirs(app_id).ok().flatten()?;
    Some(data.parent()?.join("kept-documents.json"))
}

/// [`activate`] once the session check has passed: `announce` tells the
/// session its list changed.
fn begin(
    owner: u64,
    app_id: &str,
    set: &Path,
    recents: Recents,
    announce: Arc<dyn Fn() + Send + Sync>,
) {
    kept::open(owner, app_id, set);
    // A second process of the app, or a set that does not read: nothing kept.
    if kept::list(owner).is_none() {
        return;
    }
    kept::listen(owner, Some(announce));
    let recents = Arc::new(recents);
    *RECENTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(recents.clone());
    kept::on_forgotten(Arc::new(move |identity, path| {
        let _ = recents.remove(identity, path);
    }));
    recheck(owner);
    let found: Vec<(u64, PathBuf, Option<String>)> = kept::to_restore(owner)
        .into_iter()
        .map(|r| (r.n, r.path, None))
        .collect();
    kept::restore(owner, &found);
}

fn recents() -> Option<Arc<Recents>> {
    RECENTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// What the person chose (D3), at `real`, kept for `owner`'s app and noted
/// in the recent-files store: its `doc:` path, or `None` when the app keeps
/// nothing here. A choice grants reading and writing, as the desktop's
/// file chooser does; a write still needs `fs.write doc:/`.
pub(crate) fn chosen(owner: u64, real: &Path) -> Option<String> {
    let recents = recents()?;
    let doc = kept::keep(owner, real, "", "readwrite")?;
    // A choice the store did not take is not held, and so not kept.
    let _ = recents.note(real);
    recheck(owner);
    Some(doc)
}

/// The recent-files store changed (or may have): drop what it no longer
/// holds, as when the person cleared the desktop's recent history.
pub(crate) fn recheck(owner: u64) {
    if let Some(recents) = recents() {
        kept::held(owner, &recents.held());
    }
}

/// The folder the store is in, for the document watch's inotify (a writer
/// replaces the store by a rename into it), once this process keeps.
pub(crate) fn watched_folder() -> Option<PathBuf> {
    Some(recents()?.file.parent()?.to_path_buf())
}

/// Whether a change at `path` is to the recent-files store.
pub(crate) fn is_store(path: &Path) -> bool {
    recents().is_some_and(|r| r.file == path)
}

/// The freedesktop recent-files store, as one app reads and marks it.
pub(crate) struct Recents {
    file: PathBuf,
    /// The `name` of the app's `bookmark:application`: its id.
    app: String,
    /// Its `exec`: how the desktop would open an entry with it.
    exec: String,
}

impl Recents {
    /// `$XDG_DATA_HOME/recently-used.xbel` (`~/.local/share` when unset).
    fn of(app_id: &str) -> Option<Self> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                let home = PathBuf::from(std::env::var_os("HOME")?);
                home.is_absolute().then(|| home.join(".local/share"))
            })?;
        let exe = std::env::current_exe().ok()?;
        Some(Self::at(base.join(STORE), app_id, &exe))
    }

    fn at(file: PathBuf, app_id: &str, exe: &Path) -> Self {
        Self {
            file,
            app: app_id.to_owned(),
            // GLib stores the command shell-quoted and parses what it
            // unquotes as a command line (`g_bookmark_file_get_application_info`),
            // so the executable is quoted inside it.
            exec: shell_quote(&format!("{} %u", shell_quote(&exe.to_string_lossy()))),
        }
    }

    fn read(&self) -> std::io::Result<String> {
        match std::fs::read_to_string(&self.file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            other => other,
        }
    }

    /// The paths of the entries marked with this app. A store that does
    /// not read or parse holds nothing.
    pub(crate) fn held(&self) -> Vec<PathBuf> {
        let Ok(text) = self.read() else {
            return Vec::new();
        };
        let Ok(doc) = roxmltree::Document::parse(&text) else {
            return Vec::new();
        };
        bookmarks(&doc)
            .filter(|b| mark(b, &self.app).is_some())
            .filter_map(path_of)
            .collect()
    }

    /// Note `real` as used now by this app: a new entry, or the entry
    /// already there marked with it too. A store that exists but does not
    /// parse is the person's, left as it is (and the choice not held). Past
    /// [`MOST`] entries marked with it, the oldest lose the mark.
    pub(crate) fn note(&self, real: &Path) -> std::io::Result<()> {
        let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidData);
        let href = url::Url::from_file_path(real).map_err(|()| invalid())?;
        let folder = real.is_dir();
        let now = timestamp(std::time::SystemTime::now());
        self.edit(|text, doc| {
            let mut edits = Vec::new();
            let root = doc.root_element();
            let ours = |at: &roxmltree::Node| {
                format!(
                    "<{} name=\"{}\" exec=\"{}\" modified=\"{now}\" count=\"1\"/>",
                    qualified(at, BOOKMARK_NS, "application"),
                    escape(&self.app),
                    escape(&self.exec)
                )
            };
            let found = bookmarks(doc).find(|b| b.attribute("href") == Some(href.as_str()));
            let list = found.and_then(|b| {
                b.descendants()
                    .find(|n| n.has_tag_name((BOOKMARK_NS, "applications")))
                    .filter(|l| !text[l.range()].ends_with("/>"))
            });
            match (found, found.and_then(|b| mark(&b, &self.app)), list) {
                (Some(b), Some(app), _) => {
                    let count = app.attribute("count").and_then(|c| c.parse::<u64>().ok());
                    edits.push(set_attribute(&b, "modified", &now));
                    edits.push(set_attribute(&b, "visited", &now));
                    edits.push(set_attribute(&app, "modified", &now));
                    let count = (count.unwrap_or(0) + 1).to_string();
                    edits.push(set_attribute(&app, "count", &count));
                }
                (Some(b), None, Some(list)) => {
                    edits.push(set_attribute(&b, "modified", &now));
                    edits.push(set_attribute(&b, "visited", &now));
                    let end = closing_tag(text, list.range());
                    edits.push((end..end, format!("  {}\n        ", ours(&list))));
                }
                // No list to join (not what a desktop writes): the entry is
                // written again, with this app's mark alone.
                (Some(b), None, None) => {
                    let added = b.attribute("added").unwrap_or(&now).to_owned();
                    let entry = entry(&root, &href, folder, &added, &now, &ours(&root));
                    edits.push((b.range(), entry));
                }
                (None, ..) => {
                    let entry = entry(&root, &href, folder, &now, &now, &ours(&root));
                    edits.extend(append(text, &root, entry));
                }
            }
            edits.extend(declared(&root));
            // The oldest of this app's entries past the bound, by when it
            // marked them; the one noted now is the newest.
            let mut marked: Vec<_> = bookmarks(doc)
                .filter(|b| b.attribute("href") != Some(href.as_str()))
                .filter_map(|b| Some((mark(&b, &self.app)?.attribute("modified").unwrap_or(""), b)))
                .collect();
            marked.sort_by(|a, b| b.0.cmp(a.0));
            for (_, b) in marked.into_iter().skip(MOST - 1) {
                edits.push(unmark(text, &b, &self.app));
            }
            Ok(edits)
        })
    }

    /// Take this app's mark off the entries for a document it forgot
    /// (`forgetDocument`, D5): the object `identity` wherever it is now, and
    /// whatever is at `path`, where it was last found (it may be gone, or
    /// another program's file may stand there). An entry no other app marks
    /// goes with it.
    pub(crate) fn remove(&self, identity: Identity, path: &Path) -> std::io::Result<()> {
        self.edit(|text, doc| {
            Ok(bookmarks(doc)
                .filter(|b| mark(b, &self.app).is_some())
                .filter(|b| {
                    path_of(*b).is_some_and(|p| {
                        p == path || Identity::of(&p).is_some_and(|(i, _)| i == identity)
                    })
                })
                .map(|b| unmark(text, &b, &self.app))
                .collect())
        })
    }

    /// Apply `edits` to the store: written whole to a private file and
    /// renamed into place, as GLib's `g_file_set_contents` writes it, so a
    /// reader sees the old store or the new one. GLib's writers take no
    /// lock, so the store is read again just before the rename; if another
    /// writer replaced it meanwhile, the edit is made again on what it
    /// wrote (three tries). An edit whose result does not parse is not
    /// written.
    fn edit(
        &self,
        edits: impl Fn(&str, &roxmltree::Document) -> std::io::Result<Vec<Edit>>,
    ) -> std::io::Result<()> {
        let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidData);
        let folder = self.file.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        for _ in 0..3 {
            let before = stamp(&self.file);
            let text = self.read()?;
            let text = if text.trim().is_empty() {
                EMPTY.to_owned()
            } else {
                text
            };
            let doc = roxmltree::Document::parse(&text).map_err(|_| invalid())?;
            let edits = edits(&text, &doc)?;
            if edits.is_empty() {
                return Ok(());
            }
            let out = splice(&text, edits);
            roxmltree::Document::parse(&out).map_err(|_| invalid())?;
            std::fs::create_dir_all(folder)?;
            let staged = folder.join(format!(".{STORE}.exact-{}", std::process::id()));
            let mut file = private(&staged)?;
            std::io::Write::write_all(&mut file, out.as_bytes())?;
            file.sync_all()?;
            if stamp(&self.file) != before {
                let _ = std::fs::remove_file(&staged);
                continue;
            }
            return std::fs::rename(&staged, &self.file);
        }
        Err(std::io::ErrorKind::ResourceBusy.into())
    }
}

/// The most entries of the store one app marks (GTK's
/// `gtk-recent-files-limit` was 50); the desktop's own writers prune by age.
const MOST: usize = 50;

type Edit = (std::ops::Range<usize>, String);

/// What identifies the store's file as written: another writer's rename
/// changes it.
fn stamp(file: &Path) -> Option<(u64, u64, std::time::SystemTime)> {
    let m = std::fs::metadata(file).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&m);
    #[cfg(not(unix))]
    let inode = 0;
    Some((inode, m.len(), m.modified().ok()?))
}

/// A new file only its owner reads, as GTK keeps the store (`0600`): it
/// lists every app's recent documents.
fn private(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let file = options.open(path)?;
    // A file left from an earlier try keeps its mode: set it.
    #[cfg(unix)]
    file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    Ok(file)
}

/// The edit that takes `app`'s mark off `bookmark`: the mark alone when
/// another app marks it too, else the whole entry.
fn unmark(text: &str, bookmark: &roxmltree::Node, app: &str) -> Edit {
    let others = bookmark
        .descendants()
        .filter(|n| n.has_tag_name((BOOKMARK_NS, "application")))
        .count()
        > 1;
    let gone = match mark(bookmark, app) {
        Some(mark) if others => mark.range(),
        _ => bookmark.range(),
    };
    (leading_space(text, gone.start)..gone.end, String::new())
}

/// The edit that adds `entry` as the root's last child, opening a root
/// written as an empty element (`<xbel/>`).
fn append(text: &str, root: &roxmltree::Node, entry: String) -> Vec<Edit> {
    let range = root.range();
    if text[range.clone()].ends_with("/>") {
        let close = range.end - 2;
        let name = &text[range.start + 1..range.start + 1 + tag_name_len(root)];
        return vec![(close..range.end, format!(">\n  {entry}\n</{name}>"))];
    }
    let end = closing_tag(text, range);
    vec![(end..end, format!("  {entry}\n"))]
}

/// `xmlns:bookmark` and `xmlns:mime` on the root where nothing names the
/// two namespaces there, so what [`qualified`] names is declared.
fn declared(root: &roxmltree::Node) -> Vec<Edit> {
    let at = root.range().start + 1 + tag_name_len(root);
    [("bookmark", BOOKMARK_NS), ("mime", MIME_NS)]
        .into_iter()
        .filter(|(_, ns)| root.lookup_prefix(ns).is_none())
        .map(|(prefix, ns)| (at..at, format!(" xmlns:{prefix}=\"{ns}\"")))
        .collect()
}

/// `local` in the namespace `ns`, by the prefix in scope at `node`, else by
/// the prefix [`declared`] gives it on the root.
fn qualified(node: &roxmltree::Node, ns: &str, local: &str) -> String {
    let fallback = if ns == MIME_NS { "mime" } else { "bookmark" };
    format!("{}:{local}", node.lookup_prefix(ns).unwrap_or(fallback))
}

const EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xbel version=\"1.0\"\n      xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n      xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n</xbel>\n";

fn bookmarks<'a, 'i>(
    doc: &'a roxmltree::Document<'i>,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> {
    doc.root_element()
        .children()
        .filter(|n| n.has_tag_name("bookmark"))
}

/// The bookmark's `bookmark:application` named `app`.
fn mark<'a, 'i>(bookmark: &roxmltree::Node<'a, 'i>, app: &str) -> Option<roxmltree::Node<'a, 'i>> {
    bookmark
        .descendants()
        .find(|n| n.has_tag_name((BOOKMARK_NS, "application")) && n.attribute("name") == Some(app))
}

fn path_of(bookmark: roxmltree::Node) -> Option<PathBuf> {
    url::Url::parse(bookmark.attribute("href")?)
        .ok()?
        .to_file_path()
        .ok()
}

/// One entry, as GLib writes one, its prefixes those in scope at `root`.
fn entry(
    root: &roxmltree::Node,
    href: &url::Url,
    folder: bool,
    added: &str,
    now: &str,
    application: &str,
) -> String {
    let mime = if folder {
        "inode/directory"
    } else {
        "application/octet-stream"
    };
    let (mime_type, applications) = (
        qualified(root, MIME_NS, "mime-type"),
        qualified(root, BOOKMARK_NS, "applications"),
    );
    format!(
        "<bookmark href=\"{}\" added=\"{}\" modified=\"{now}\" visited=\"{now}\">\n    <info>\n      <metadata owner=\"http://freedesktop.org\">\n        <{mime_type} type=\"{mime}\"/>\n        <{applications}>\n          {application}\n        </{applications}>\n      </metadata>\n    </info>\n  </bookmark>",
        escape(href.as_str()),
        escape(added),
    )
}

/// An edit that gives `node`'s attribute `name` the value `value`, adding
/// it after the element's name when it has none.
fn set_attribute(
    node: &roxmltree::Node,
    name: &str,
    value: &str,
) -> (std::ops::Range<usize>, String) {
    match node
        .attributes()
        .find(|a| a.name() == name && a.namespace().is_none())
    {
        Some(a) => (a.range_value(), escape(value)),
        None => {
            let at = node.range().start + 1 + tag_name_len(node);
            (at..at, format!(" {name}=\"{}\"", escape(value)))
        }
    }
}

fn tag_name_len(node: &roxmltree::Node) -> usize {
    let source = &node.document().input_text()[node.range().start + 1..];
    source
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(0)
}

/// Where the closing tag of the element spanning `range` starts.
fn closing_tag(text: &str, range: std::ops::Range<usize>) -> usize {
    text[range.clone()]
        .rfind("</")
        .map_or(range.end, |i| range.start + i)
}

/// The start of the whitespace before `at`, so a removed element leaves no
/// blank line.
fn leading_space(text: &str, at: usize) -> usize {
    at - (text[..at].len() - text[..at].trim_end().len())
}

/// `edits` applied to `text`; none overlap.
fn splice(text: &str, mut edits: Vec<(std::ops::Range<usize>, String)>) -> String {
    edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
    let mut out = text.to_owned();
    for (range, with) in edits {
        out.replace_range(range, &with);
    }
    out
}

/// `g_shell_quote`: in single quotes, each quote inside written `'\''`.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// ISO 8601 in UTC with microseconds, as GLib writes the store's times.
fn timestamp(at: std::time::SystemTime) -> String {
    let since = at.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let seconds = since.as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970 as a civil date (Howard Hinnant's `civil_from_days`).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:06}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        since.subsec_micros()
    )
}

#[cfg(test)]
#[path = "kept_tests.rs"]
mod tests;
