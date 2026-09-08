//! The LLP reader's data source.
//!
//! The same parser and the same blocks as `apps/markdown` — this app is the
//! general reader specialised, not a second one — plus the four things a
//! corpus has that a folder of files does not: an index in number order
//! with sub-documents under their parents, a search over every document's
//! text, an outline of the open document, and `LLP 1234` in running prose
//! resolved to the document it names.
//!
//! Reading the corpus is I/O, so it happens on the host's worker through a
//! continuation (LLP 1016 D1). It happens once: the index keeps every
//! document's text, and a search after that is a filter over memory rather
//! than a walk over a disk.
//!
//! @ref LLP 1033

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Store};
use markdown_parse::{parse, Document, Kind};

mod index;
mod refs;

/// Read a corpus directory: every document, in number order. The reader
/// calls this on the host's worker; a test calls it directly.
pub use index::read as read_index;
pub use index::Entry;

/// A corpus that was read, or the reason it was not.
type Corpus = Result<Vec<Entry>, String>;

/// The LLP reader's data source.
#[derive(Default)]
pub struct Llp {
    /// The corpus, by the root it was read from.
    corpus: HashMap<String, Vec<Entry>>,
    /// The document open now, parsed: what a refusal falls back to.
    open: Option<(PathBuf, Document)>,
    /// Roots the worker has finished, by the root that was asked for.
    done: Arc<Mutex<HashMap<String, Corpus>>>,
    /// Roots handed out as continuation tokens and not yet taken.
    inflight: HashMap<u64, String>,
    next: u64,
}

impl Llp {
    /// A new source with nothing read.
    pub fn new() -> Llp {
        Llp::default()
    }

    /// The directory a path belongs to: itself when it is one, its folder
    /// when it is a file. This is the corpus's root and the index's subject.
    fn root_of(asked: &str) -> String {
        if asked.is_empty() {
            return String::new();
        }
        let path = Path::new(asked);
        if path.is_dir() {
            return asked.to_string();
        }
        path.parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The document a path names: itself when it is a file, the corpus's
    /// lowest-numbered document when it is a directory — which is how
    /// `llpview llp/` lands on the root explainer and not on nothing.
    fn document_of(&self, asked: &str) -> Option<PathBuf> {
        let path = PathBuf::from(asked);
        if path.is_file() {
            return Some(path);
        }
        let entries = self.corpus.get(&Self::root_of(asked))?;
        entries.first().map(|e| e.path.clone())
    }

    fn entries(&self, root: &str) -> &[Entry] {
        self.corpus.get(root).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// `shape Index`: what the sidebar lists.
///
/// `query` filters over every document's text; `overlay` narrows to a
/// declared working set (`current`, `foundation`) or shows everything;
/// `newest` puts the highest number first, which is the end of a corpus
/// people actually read from.
fn index_value(
    root: &str,
    entries: &[Entry],
    query: &str,
    overlay: &str,
    newest: bool,
    open: Option<&Path>,
) -> Value {
    let query = query.trim().to_lowercase();
    let mut shown: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.matches(&query))
        .filter(|e| overlay.is_empty() || e.overlay == overlay)
        .collect();
    if newest {
        // Reversed whole, so a sub-document still follows its parent: the
        // nesting is the point of the order, not the direction.
        shown.reverse();
    }
    Value::record(vec![
        Value::str(root),
        // The folder the corpus lives in — what the window is titled by, and
        // what tells one repository's `llp/` from another's at a glance.
        Value::str(
            &Path::new(root)
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        Value::Number(entries.len() as f64),
        Value::Number(shown.len() as f64),
        Value::list(
            shown
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    Value::record(vec![
                        Value::str(&i.to_string()),
                        Value::str(&e.number),
                        Value::str(&e.title),
                        Value::str(&e.path.to_string_lossy()),
                        Value::str(&e.kind),
                        Value::str(&e.status),
                        Value::Number(e.depth as f64),
                        Value::str(&e.overlay),
                        Value::Bool(open == Some(e.path.as_path())),
                        Value::Number(e.hits(&query) as f64),
                    ])
                })
                .collect(),
        ),
    ])
}

/// The blocks of one section: the heading at `from` and everything under it
/// until the next heading at the same level or above. `-1` is the whole
/// document. There is no scrolling to an anchor in v1, so the outline
/// *shows* a section rather than jumping to it — which for a document of
/// 16,000 words is the better of the two anyway.
fn section(doc: &Document, from: i64) -> Document {
    if from < 0 {
        return doc.clone();
    }
    let start = from as usize;
    let Some(head) = doc.blocks.get(start) else {
        return doc.clone();
    };
    let level = head.depth;
    let end = doc.blocks[start + 1..]
        .iter()
        .position(|b| b.kind == Kind::Heading && b.depth <= level)
        .map(|offset| start + 1 + offset)
        .unwrap_or(doc.blocks.len());
    Document {
        title: doc.title.clone(),
        blocks: doc.blocks[start..end].to_vec(),
    }
}

/// `shape Heading`: the outline, every heading of the whole document.
fn outline_value(doc: &Document, showing: i64) -> Value {
    Value::list(
        doc.outline()
            .iter()
            .map(|(level, text, at)| {
                Value::record(vec![
                    Value::str(&at.to_string()),
                    Value::str(text),
                    Value::Number(*level as f64),
                    Value::Number(*at as f64),
                    Value::Bool(showing == *at as i64),
                ])
            })
            .collect(),
    )
}

/// `shape Doc`.
fn document_value(
    path: &Path,
    whole: &Document,
    entry: Option<&Entry>,
    showing: i64,
    message: &str,
) -> Value {
    let shown = section(whole, showing);
    Value::record(vec![
        Value::str(&path.to_string_lossy()),
        Value::str(
            &path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
        Value::str(entry.map(|e| e.number.as_str()).unwrap_or("")),
        Value::str(&whole.title),
        Value::str(entry.map(|e| e.kind.as_str()).unwrap_or("")),
        Value::str(entry.map(|e| e.status.as_str()).unwrap_or("")),
        Value::str(message),
        Value::Bool(!message.is_empty()),
        Value::Number(showing as f64),
        Value::Number(shown.blocks.len() as f64),
        markdown_parse::value::blocks(&shown),
        outline_value(whole, showing),
    ])
}

fn nothing_open(message: &str) -> Value {
    document_value(Path::new(""), &Document::default(), None, -1, message)
}

impl DataSource for Llp {
    fn app_id(&self) -> &str {
        "com.exact.llp"
    }

    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        // The bake has no corpus to read: the first frame is an empty index
        // and the welcome document, and the real one arrives after it.
        match source {
            "theme" => Ok(markdown_parse::theme::value()),
            "index" => Ok(index_value("", &[], "", "", false, None)),
            "document" => {
                let doc = parse(WELCOME, &|t: &str| t.to_string());
                Ok(document_value(Path::new("LLP"), &doc, None, -1, ""))
            }
            other => Err(DataError::UnknownSource(other.to_string())),
        }
    }

    fn answer(
        &mut self,
        _store: &mut Store,
        source: &str,
        args: &[Value],
    ) -> Result<Answer, DataError> {
        // The palette is one constant: every token is a `light-dark()` pair
        // and the host resolves it, so this takes no appearance and is never
        // re-asked when the scheme changes (LLP 1034 D1/D3).
        if source == "theme" {
            return Ok(Answer::Now(markdown_parse::theme::value()));
        }
        let asked = match args.first() {
            Some(Value::Str(s)) => s.to_string(),
            _ => return Err(DataError::BadArguments(format!("{source}(path, …)"))),
        };
        if asked.is_empty() {
            return self.query(source, args).map(Answer::Now);
        }
        let root = Self::root_of(&asked);
        // Whatever is asked for, the corpus comes first: the index is the
        // sidebar, the search, *and* the cross-reference table, so nothing
        // here can be answered without it.
        if !self.corpus.contains_key(&root) {
            if let Some(result) = self.done.lock().ok().and_then(|mut d| d.remove(&root)) {
                match result {
                    Ok(entries) => {
                        self.corpus.insert(root.clone(), entries);
                    }
                    Err(message) => return Ok(Answer::Now(self.refuse(source, &message))),
                }
            } else {
                self.next += 1;
                self.inflight.insert(self.next, root);
                return Ok(Answer::Later(Request::continuation(self.next)));
            }
        }
        Ok(Answer::Now(self.value(source, &asked, args)))
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let root = self.inflight.remove(&token)?;
        let done = Arc::clone(&self.done);
        Some(Box::new(move || {
            let result = index::read(Path::new(&root));
            if let Ok(mut map) = done.lock() {
                map.insert(root, result);
            }
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        }))
    }

    fn parse(
        &mut self,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if let Outcome::Failed { kind, message } = &outcome {
            let asked = match args.first() {
                Some(Value::Str(s)) => s.to_string(),
                _ => String::new(),
            };
            if self
                .done
                .lock()
                .ok()
                .map(|d| !d.contains_key(&Self::root_of(&asked)))
                == Some(true)
            {
                let message = match kind {
                    FailureKind::Unsupported => {
                        "this surface cannot read a folder of documents".to_string()
                    }
                    _ => message.clone(),
                };
                return Ok(Answer::Now(self.refuse(source, &message)));
            }
        }
        // The corpus is in hand (or refused): answering is the same path it
        // was before the worker ran.
        self.answer(store, source, args)
    }
}

impl Llp {
    /// The value of a source once the corpus is in hand.
    fn value(&mut self, source: &str, asked: &str, args: &[Value]) -> Value {
        let root = Self::root_of(asked);
        match source {
            "index" => {
                let text = |i: usize| match args.get(i) {
                    Some(Value::Str(s)) => s.to_string(),
                    _ => String::new(),
                };
                let newest = matches!(args.get(3), Some(Value::Bool(true)));
                let open = self.document_of(asked);
                index_value(
                    &root,
                    self.entries(&root),
                    &text(1),
                    &text(2),
                    newest,
                    open.as_deref(),
                )
            }
            "document" => {
                let showing = match args.get(1) {
                    Some(Value::Number(n)) => *n as i64,
                    _ => -1,
                };
                let Some(path) = self.document_of(asked) else {
                    return nothing_open(&format!("{asked} holds no Markdown documents"));
                };
                if self.open.as_ref().map(|(p, _)| p != &path).unwrap_or(true) {
                    match self.read(&path, &root) {
                        Ok(doc) => self.open = Some((path.clone(), doc)),
                        Err(message) => {
                            return match &self.open {
                                Some((open, doc)) => {
                                    let entry = self.entry(&root, open);
                                    document_value(open, doc, entry, showing, &message)
                                }
                                None => nothing_open(&message),
                            }
                        }
                    }
                }
                let (open, doc) = self.open.as_ref().expect("just opened");
                let entry = self.entry(&root, open);
                document_value(open, doc, entry, showing, "")
            }
            other => nothing_open(&format!("no source {other}")),
        }
    }

    fn entry<'a>(&'a self, root: &str, path: &Path) -> Option<&'a Entry> {
        self.entries(root).iter().find(|e| e.path == path)
    }

    /// Parse one document, with `LLP 1234` in its prose resolved against the
    /// corpus — the reference that makes a corpus navigable rather than a
    /// pile of files.
    fn read(&self, path: &Path, root: &str) -> Result<Document, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut doc = parse(&text, &|target: &str| resolve(&base, target));
        let entries = self.entries(root);
        for block in &mut doc.blocks {
            block.runs = refs::link(std::mem::take(&mut block.runs), &|number| {
                entries
                    .iter()
                    .find(|e| e.number == number)
                    .map(|e| e.path.to_string_lossy().into_owned())
            });
            for cell in &mut block.cells {
                *cell = refs::link(std::mem::take(cell), &|number| {
                    entries
                        .iter()
                        .find(|e| e.number == number)
                        .map(|e| e.path.to_string_lossy().into_owned())
                });
            }
        }
        Ok(doc)
    }

    fn refuse(&mut self, source: &str, message: &str) -> Value {
        match source {
            "index" => index_value("", &[], "", "", false, None),
            _ => nothing_open(message),
        }
    }
}

/// A link target as the app should act on it (the general reader's rule,
/// and the same one).
pub fn resolve(base: &Path, target: &str) -> String {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return target.to_string();
    }
    let without_anchor = target.split('#').next().unwrap_or(target);
    let joined = base.join(without_anchor);
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for part in joined.components() {
        match part {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::CurDir => {}
            other => parts.push(other.as_os_str().to_os_string()),
        }
    }
    let mut out = PathBuf::new();
    for part in parts {
        out.push(part);
    }
    out.to_string_lossy().into_owned()
}

/// What the reader shows before a corpus is open.
pub const WELCOME: &str = r#"# LLP

A reader for a directory of LLP documents: `NNNN-slug.type.md`, numbered,
with sub-documents under their parents and `current/` and `foundation/` as
the declared working set.

Point it at one:

```sh
llpview ~/projects/exact2/llp
```

The index is on the left, in number order. Search runs over every
document's text, not only its title. The outline on the right shows one
section at a time — a 16,000-word document is easier read a section at a
time than scrolled. `LLP 1234` in running prose is a link to that document.
"#;

/// The runs of a document, with references linked.
pub use refs::link as link_references;
