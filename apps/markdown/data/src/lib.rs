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
    /// The document being read now: what a refusal falls back to.
    open: Option<(PathBuf, Document)>,
    /// Results the worker has finished, by the path that was asked for.
    done: Arc<Mutex<HashMap<String, Opened>>>,
    /// Paths handed out as continuation tokens and not yet taken.
    inflight: HashMap<u64, String>,
    next: u64,
}

impl Markdown {
    /// A new source with nothing open.
    pub fn new() -> Markdown {
        Markdown::default()
    }

    /// `shape Document`: the file, what is beside it, and any refusal.
    fn value(&self, path: &Path, doc: &Document, message: &str) -> Value {
        Value::record(vec![
            Value::str(&path.to_string_lossy()),
            Value::str(&name_of(path)),
            Value::str(&doc.title),
            Value::str(message),
            Value::Bool(!message.is_empty()),
            Value::Number(doc.blocks.len() as f64),
            markdown_parse::value::blocks(doc),
            siblings(path),
        ])
    }

    /// The value of `open(path)` once the file is in hand.
    fn opened(&mut self, asked: &str, result: Opened) -> Value {
        match result {
            Ok((path, doc)) => {
                let value = self.value(&path, &doc, "");
                self.open = Some((path, doc));
                value
            }
            // The refusal, over the document still being read (never over
            // nothing: losing the page you were on is the worse failure).
            Err(message) => match self.open.take() {
                Some((path, doc)) => {
                    let value = self.value(&path, &doc, &message);
                    self.open = Some((path, doc));
                    value
                }
                None => self.value(Path::new(asked), &Document::default(), &message),
            },
        }
    }

    /// The welcome document: what `open("")` is, and the one document a
    /// surface that cannot reach a filesystem can still show.
    fn welcome(&mut self) -> Value {
        let doc = parse(WELCOME, &|target: &str| target.to_string());
        let value = self.value(Path::new("Markdown"), &doc, "");
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
    // Text, not bytes: a file that is not UTF-8 is not this reader's, and
    // saying so is better than showing replacement characters.
    let text =
        String::from_utf8(bytes).map_err(|_| format!("{} is not UTF-8 text", name_of(&path)))?;
    let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let doc = parse(&text, &|target: &str| resolve(&base, target));
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
        if asked.is_empty() {
            return Ok(Answer::Now(self.welcome()));
        }
        if let Some(result) = self.done.lock().ok().and_then(|mut d| d.remove(&asked)) {
            return Ok(Answer::Now(self.opened(&asked, result)));
        }
        self.next += 1;
        self.inflight.insert(self.next, asked);
        Ok(Answer::Later(Request::continuation(self.next)))
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let asked = self.inflight.remove(&token)?;
        let done = Arc::clone(&self.done);
        Some(Box::new(move || {
            let result = read(&asked);
            if let Ok(mut map) = done.lock() {
                map.insert(asked, result);
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
        let result = self.done.lock().ok().and_then(|mut d| d.remove(&asked));
        let result = match (result, outcome) {
            (Some(result), _) => result,
            // The worker never ran it: a surface with no filesystem (a
            // browser) is the ordinary case, so it reads as a refusal and
            // not as a defect.
            (None, Outcome::Failed { kind, message }) => Err(match kind {
                FailureKind::Unsupported => "this surface cannot open local files".to_string(),
                _ => message,
            }),
            (None, Outcome::Response(_)) => Err(format!("{asked} was not read")),
        };
        Ok(Answer::Now(self.opened(&asked, result)))
    }
}
