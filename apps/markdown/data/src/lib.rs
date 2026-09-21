//! Markdown's data source: a path in, a document out.
//!
//! The runner does no I/O (LLP 1016 D1), so opening a file is a request the
//! host runs: `answer` hands back a continuation token, the host's worker
//! reads *and parses* the file, and `parse` picks up the result. Parsing on
//! the worker and not here is the point — a 100 kB document is milliseconds
//! of work, and milliseconds on the thread that lays out is a dropped frame.
//!
//! This source owns *the open document*, which is why a file that will not
//! open does not lose the one being read: a refusal comes back as the
//! document already open plus a message, and the reader shows both.
//!
//! @ref LLP 1033 (the reader), LLP 1016 D1/D2 (requests as values)

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Store};
use markdown_parse::{parse, Document};

mod welcome;

/// What the reader shows before anything is opened, and what the web build
/// shows always: a real document, parsed by the same parser.
pub use welcome::WELCOME;

/// The largest file the reader opens. Past this a Markdown file is not
/// prose, and reading it would stall the worker for no one's benefit.
pub const LIMIT: u64 = 4 * 1024 * 1024;

/// A file the reader opened, or could not.
type Opened = Result<(PathBuf, Document), String>;

/// The Markdown reader's data source.
#[derive(Default)]
pub struct Markdown {
    /// The document value being read now, shared with the runner. A refusal
    /// reuses its blocks rather than retaining the parser's second text copy.
    open: Option<(PathBuf, Value)>,
    /// Results the worker has finished, by the path that was asked for.
    done: Arc<Mutex<HashMap<String, (u64, Opened)>>>,
    /// Paths handed out as continuation tokens and not yet taken.
    inflight: HashMap<u64, String>,
    next: u64,
    /// Newest open, published before its continuation joins the ordered lane.
    generation: Arc<AtomicU64>,
}

impl Markdown {
    /// A new source with nothing open.
    pub fn new() -> Markdown {
        Markdown::default()
    }

    /// `shape Document`: the file, what is beside it, and any refusal.
    fn value(path: &Path, doc: Document, message: &str) -> Value {
        Value::record(vec![
            Value::str(&path.to_string_lossy()),
            Value::str(&name_of(path)),
            Value::str(&doc.title),
            Value::str(message),
            Value::Bool(!message.is_empty()),
            Value::Number(doc.blocks.len() as f64),
            markdown_parse::value::into_blocks(doc),
            siblings(path),
        ])
    }

    /// The value of `open(path)` once the file is in hand.
    fn opened(&mut self, asked: &str, result: Opened) -> Value {
        match result {
            Ok((path, doc)) => {
                let value = Self::value(&path, doc, "");
                self.open = Some((path, value.clone()));
                value
            }
            // The refusal, over the document still being read (never over
            // nothing: losing the page you were on is the worse failure).
            Err(message) => match &self.open {
                Some((path, Value::Record(fields))) => {
                    let mut fields = fields.as_ref().clone();
                    fields[3] = Value::str(&message);
                    fields[4] = Value::Bool(!message.is_empty());
                    fields[7] = siblings(path);
                    Value::record(fields)
                }
                _ => Self::value(Path::new(asked), Document::default(), &message),
            },
        }
    }

    /// The welcome document: what `open("")` is, and the one document a
    /// surface that cannot reach a filesystem can still show.
    fn welcome(&mut self) -> Value {
        let doc = parse(WELCOME, &|target: &str| target.to_string());
        let value = Self::value(Path::new("Markdown"), doc, "");
        self.open = None;
        value
    }
}

/// The Markdown files beside this one, in name order, the open one marked —
/// the only navigation a one-document reader needs, and the reason opening
/// a file in a folder of notes is not a dead end.
fn siblings(path: &Path) -> Value {
    let Some(folder) = path.parent().filter(|p| !p.as_os_str().is_empty()) else {
        return Value::list(Vec::new());
    };
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Value::list(Vec::new());
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("md") | Some("markdown")
            )
        })
        .collect();
    found.sort();
    Value::list(
        found
            .iter()
            .take(500)
            .enumerate()
            .map(|(i, p)| {
                Value::record(vec![
                    Value::str(&i.to_string()),
                    Value::str(&name_of(p)),
                    Value::str(&p.to_string_lossy()),
                    Value::Bool(p == path),
                ])
            })
            .collect(),
    )
}

/// What to put in the window for this path.
pub fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Read and parse one path on the host's worker. A directory opens the
/// README beside it, the way a repository page does on the web.
pub fn read(asked: &str) -> Opened {
    read_cancellable(asked, &|| false)
}

fn read_cancellable(asked: &str, cancel: &dyn Fn() -> bool) -> Opened {
    if cancel() {
        return Err("file open superseded".into());
    }
    let path = PathBuf::from(asked);
    let path = if path.is_dir() {
        ["README.md", "readme.md", "index.md", "README.markdown"]
            .iter()
            .map(|n| path.join(n))
            .find(|p| p.is_file())
            .ok_or_else(|| format!("{asked} is a folder with no README.md in it"))?
    } else {
        path
    };
    let size = std::fs::metadata(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .len();
    if size > LIMIT {
        return Err(format!(
            "{} is {} MB — larger than this reader opens",
            name_of(&path),
            size / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if cancel() {
        return Err("file open superseded".into());
    }
    // Text, not bytes: a file that is not UTF-8 is not this reader's, and
    // saying so is better than showing replacement characters.
    let text =
        String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text", name_of(&path)))?;
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let doc =
        markdown_parse::parse_cancellable(&text, &|target: &str| resolve(&base, target), cancel)
            .ok_or_else(|| "file open superseded".to_string())?;
    Ok((path, doc))
}

/// A link target as the app should act on it: a page stays a URL, an anchor
/// stays an anchor, and a relative path becomes the absolute one it names —
/// resolved here, beside the document, because nothing downstream knows
/// where the document came from.
pub fn resolve(base: &Path, target: &str) -> String {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return target.to_string();
    }
    let without_anchor = target.split('#').next().unwrap_or(target);
    let path = Path::new(without_anchor);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    // Lexical, not `canonicalize`: a link to a file that does not exist yet
    // must still come out as the path it names, so the refusal can say so.
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

impl DataSource for Markdown {
    fn app_id(&self) -> &str {
        "com.exact.markdown"
    }

    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        // Every source here reaches a filesystem, so `answer` is where the
        // work is; `query` exists for the bake, which has no filesystem and
        // needs the first frame's value. @ref LLP 1027 D4
        match source {
            "open" => Ok(self.welcome()),
            "theme" => Ok(markdown_parse::theme::value()),
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
        if source != "open" {
            return Err(DataError::UnknownSource(source.to_string()));
        }
        let Some(Value::Str(asked)) = args.first() else {
            return Err(DataError::BadArguments("open(path)".into()));
        };
        let asked = asked.to_string();
        self.next += 1;
        self.generation.store(self.next, Ordering::Release);
        self.inflight.clear();
        if let Ok(mut done) = self.done.lock() {
            done.clear();
        }
        if asked.is_empty() {
            return Ok(Answer::Now(self.welcome()));
        }
        self.inflight.insert(self.next, asked);
        Ok(Answer::Later(Request::continuation(self.next)))
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let asked = self.inflight.remove(&token)?;
        let done = Arc::clone(&self.done);
        let generation = Arc::clone(&self.generation);
        Some(Box::new(move || {
            let cancel = || generation.load(Ordering::Acquire) != token;
            let result = read_cancellable(&asked, &cancel);
            if let Ok(mut map) = done.lock() {
                // Check while holding the result lock: a superseding answer
                // clears it under the same lock. A→B→A must not accept old A.
                if !cancel() {
                    map.insert(asked, (token, result));
                }
            }
            if cancel() {
                return Outcome::Failed {
                    kind: FailureKind::Aborted,
                    message: "file open superseded".into(),
                };
            }
            // The reply carries nothing: the document went into `done`
            // whole, rather than through a serialization and back.
            Outcome::Response(Response {
                status: 200,
                headers: Vec::new(),
                body: Vec::new(),
            })
        }))
    }

    fn parse(
        &mut self,
        _store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        if source != "open" {
            return Err(DataError::UnknownSource(source.to_string()));
        }
        let asked = match args.first() {
            Some(Value::Str(s)) => s.to_string(),
            _ => String::new(),
        };
        let result = self
            .done
            .lock()
            .ok()
            .and_then(|mut d| d.remove(&asked))
            .filter(|(generation, _)| *generation == self.next)
            .map(|(_, result)| result);
        let result = match (result, outcome) {
            (Some(result), _) => result,
            // The worker never ran it: a surface with no filesystem (a
            // browser) is the ordinary case, so it reads as a refusal and
            // not as a defect.
            (None, Outcome::Failed { kind, message }) => Err(match kind {
                FailureKind::Unsupported => "this surface cannot open local files".to_string(),
                _ => message,
            }),
            (None, Outcome::Response(_) | Outcome::Storage(_)) => {
                Err(format!("{asked} was not read"))
            }
        };
        Ok(Answer::Now(self.opened(&asked, result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_share_the_open_value_and_switching_releases_it() {
        use std::rc::Rc;
        let directory =
            std::env::temp_dir().join(format!("exact-markdown-retain-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("first.md");
        std::fs::write(&path, "# First\n\nRead **this** [link](next.md).").unwrap();
        let mut source = Markdown::new();
        let opened = source.opened(path.to_str().unwrap(), read(path.to_str().unwrap()));
        let Value::Record(fields) = &opened else {
            panic!("expected document");
        };
        let Value::List(blocks) = &fields[6] else {
            panic!("expected blocks");
        };
        let weak = Rc::downgrade(blocks);
        let original = opened.to_bytes();
        let next = directory.join("next.md");
        std::fs::write(&next, "# Next\n\nAnother page.").unwrap();
        for message in ["not UTF-8", "too large", "missing"] {
            let refused = source.opened("/missing.md", Err(message.into()));
            let Value::Record(refusal) = &refused else {
                panic!("expected refusal over document");
            };
            let Value::List(retained) = &refusal[6] else {
                panic!("expected retained blocks");
            };
            assert!(Rc::ptr_eq(blocks, retained));
            assert_eq!(refusal[3].as_str(), Some(message));
            assert_eq!(refusal[4].as_bool(), Some(true));
            for i in [0, 1, 2, 5, 6] {
                assert_eq!(refusal[i], fields[i]);
            }
            assert_eq!(refusal[7], siblings(&path), "refresh the folder on refusal");
            assert_ne!(refusal[7], fields[7]);
            assert_eq!(opened.to_bytes(), original, "never mutate a reader's value");
        }
        drop(opened);
        assert!(
            weak.upgrade().is_some(),
            "the fallback owns the open blocks"
        );
        let latest = source.opened(next.to_str().unwrap(), read(next.to_str().unwrap()));
        assert!(
            weak.upgrade().is_none(),
            "a successful switch releases old blocks"
        );
        let Value::Record(fields) = latest else {
            panic!("expected new document");
        };
        assert_eq!(fields[2].as_str(), Some("Next"));
        source.welcome();
        let Value::Record(empty) = source.opened("/missing.md", Err("missing".into())) else {
            panic!("expected refusal without a file");
        };
        assert_eq!(empty[5].as_number(), Some(0.0));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_running_scan_releases_the_lane_for_the_newest_file() {
        use std::sync::mpsc::sync_channel;
        let directory =
            std::env::temp_dir().join(format!("exact-markdown-cancel-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let old = directory.join("old.md");
        let new = directory.join("new.md");
        std::fs::write(&old, "[x".repeat(512 * 1024)).unwrap();
        std::fs::write(&new, "# Latest\n\nReady.").unwrap();
        let mut source = Markdown::new();
        let token = open(&mut source, old.to_str().unwrap());
        let generation = Arc::clone(&source.generation);
        let (started, checkpoint) = sync_channel(0);
        let (resume, resumed) = sync_channel(0);
        let (queued, next) = sync_channel::<Box<dyn FnOnce() -> Outcome + Send>>(0);
        // One worker, just as in the host's ordered continuation lane. Pause
        // at a real scanner checkpoint so the supersession is deterministic.
        let worker = std::thread::spawn(move || {
            let calls = std::cell::Cell::new(0);
            let result = read_cancellable(old.to_str().unwrap(), &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 8 {
                    started.send(()).unwrap();
                    resumed.recv().unwrap();
                }
                generation.load(Ordering::Acquire) != token
            });
            assert_eq!(result.unwrap_err(), "file open superseded");
            assert_eq!(
                calls.get(),
                8,
                "the stopped scan must not resume visiting input"
            );
            next.recv().unwrap()()
        });
        checkpoint.recv().unwrap();
        let latest = open(&mut source, new.to_str().unwrap());
        resume.send(()).unwrap();
        queued.send(source.continuation(latest).unwrap()).unwrap();
        let outcome = worker.join().unwrap();
        source
            .parse(
                &mut Store::default(),
                "open",
                &[Value::str(new.to_str().unwrap())],
                outcome,
            )
            .unwrap();
        let Value::Record(fields) = &source.open.as_ref().unwrap().1 else {
            panic!("expected document");
        };
        assert_eq!(fields[2].as_str(), Some("Latest"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn open(source: &mut Markdown, path: &str) -> u64 {
        let answer = source
            .answer(&mut Store::default(), "open", &[Value::str(path)])
            .unwrap();
        let Answer::Later(request) = answer else {
            panic!("expected continuation")
        };
        request.continuation.unwrap()
    }

    #[test]
    fn superseded_continuations_abort_even_when_the_path_is_reopened() {
        let mut source = Markdown::new();
        let first = open(&mut source, "/missing/A.md");
        let first = source.continuation(first).unwrap();
        let second = open(&mut source, "/missing/B.md");
        let second = source.continuation(second).unwrap();
        let latest = open(&mut source, "/missing/A.md");
        let latest = source.continuation(latest).unwrap();
        for stale in [first, second] {
            assert!(matches!(
                stale(),
                Outcome::Failed {
                    kind: FailureKind::Aborted,
                    ..
                }
            ));
            assert!(source.done.lock().unwrap().is_empty());
        }
        assert!(matches!(latest(), Outcome::Response(_)));
        let mut done = source.done.lock().unwrap();
        let (generation, result) = done.remove("/missing/A.md").unwrap();
        assert_eq!(generation, source.next);
        assert!(
            result.is_err(),
            "the current file's I/O refusal is still delivered"
        );
    }

    #[test]
    fn welcome_supersedes_pending_reads_and_releases_completed_results() {
        let mut source = Markdown::new();
        let token = open(&mut source, "/missing/A.md");
        let stale = source.continuation(token).unwrap();
        source
            .answer(&mut Store::default(), "open", &[Value::str("")])
            .unwrap();
        assert!(matches!(
            stale(),
            Outcome::Failed {
                kind: FailureKind::Aborted,
                ..
            }
        ));
        let token = open(&mut source, "/missing/A.md");
        source.continuation(token).unwrap()();
        assert_eq!(source.done.lock().unwrap().len(), 1);
        open(&mut source, "/missing/B.md");
        assert!(source.done.lock().unwrap().is_empty());
    }
}
