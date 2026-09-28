//! Markdown's data source: a document path in, a document out.
//!
//! The runner does no I/O (LLP 1016 D1), so opening a file is a series of
//! storage requests the host runs (LLP 1027.001): `fs.stat` the path, the
//! README beside it when it is a folder, `fs.readFile`, and `fs.readdir`
//! for the Markdown files beside it. What it opens is a `doc:` path (LLP
//! 1069.010 D1): the file or folder the person chose, minted by the host,
//! read under `fs.read doc:/`. A path the host never minted is not the
//! app's to read, on any host. Parsing then runs on the host's worker, not
//! here: a 100 kB document is milliseconds of work, and milliseconds on the
//! thread that lays out is a dropped frame. A browser has no such worker,
//! so there the parse is inline.
//!
//! This source owns *the open document*, which is why a file that will not
//! open does not lose the one being read: a refusal comes back as the
//! document already open plus a message, and the reader shows both.
//!
//! @ref LLP 1033 (the reader), LLP 1016 D1/D2 (requests as values)

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use exact_data::storage;
use exact_plan::Value;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Response, Store};
use markdown_parse::{parse, Document};
use serde_json::{json, Value as Json};

mod welcome;

/// What the reader shows before anything is opened, and what the web build
/// shows always: a real document, parsed by the same parser.
pub use welcome::WELCOME;

/// The largest file the reader opens. Past this a Markdown file is not
/// prose, and reading it would stall the worker for no one's benefit.
pub const LIMIT: u64 = 4 * 1024 * 1024;

/// What the reader may reach: the documents the person chose, to read.
pub const GRANTS: &str = "fs.read doc:/";

/// A file the reader opened, or could not.
type Opened = Result<(String, Document), String>;

/// Where an open is, between the storage requests it makes.
enum Step {
    /// `fs.stat` of what was asked for.
    Stat,
    /// `fs.readdir` of a folder, for its README.
    Readme,
    /// `fs.readFile` of the file.
    Read { file: String },
    /// `fs.readdir` of the file's folder, for the list beside it.
    Siblings { file: String, text: String },
}

/// The Markdown reader's data source.
#[derive(Default)]
pub struct Markdown {
    /// The document value being read now, shared with the runner. A refusal
    /// reuses its blocks rather than retaining the parser's second text copy.
    open: Option<(String, Value)>,
    /// Results the worker has finished, by the path that was asked for.
    done: Arc<Mutex<HashMap<String, (u64, Opened)>>>,
    /// Texts handed to the worker as continuation tokens and not yet taken:
    /// the path asked for, the file, its text.
    inflight: HashMap<u64, (String, String, String)>,
    /// The open in progress: its generation, what was asked, its step.
    step: Option<(u64, String, Step)>,
    /// The list beside the file being parsed, attached when it is done.
    beside: Value,
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
    fn value(path: &str, doc: Document, message: &str, siblings: Value) -> Value {
        Value::record(vec![
            Value::str(path),
            Value::str(&name_of(path)),
            Value::str(&doc.title),
            Value::str(message),
            Value::Bool(!message.is_empty()),
            Value::Number(doc.blocks.len() as f64),
            markdown_parse::value::into_blocks(doc),
            siblings,
        ])
    }

    /// The value of `open(path)` once the file is in hand.
    fn opened(&mut self, asked: &str, result: Opened) -> Value {
        let siblings = std::mem::replace(&mut self.beside, Value::list(Vec::new()));
        match result {
            Ok((path, doc)) => {
                let value = Self::value(&path, doc, "", siblings);
                self.open = Some((path, value.clone()));
                value
            }
            // The refusal, over the document still being read (never over
            // nothing: losing the page you were on is the worse failure).
            Err(message) => match &self.open {
                Some((_, Value::Record(fields))) => {
                    let mut fields = fields.to_vec();
                    fields[3] = Value::str(&message);
                    fields[4] = Value::Bool(!message.is_empty());
                    Value::record(fields)
                }
                _ => Self::value(asked, Document::default(), &message, siblings),
            },
        }
    }

    /// The welcome document: what `open("")` is, and the one document a
    /// surface that cannot reach a filesystem can still show.
    fn welcome(&mut self) -> Value {
        let doc = parse(WELCOME, &|target: &str| target.to_string());
        let value = Self::value("Markdown", doc, "", Value::list(Vec::new()));
        self.open = None;
        value
    }

    /// The next storage request of an open, with the step it answers.
    fn request(&mut self, asked: &str, step: Step, op: &str, path: &str) -> Answer {
        self.step = Some((self.next, asked.to_owned(), step));
        Answer::Later(storage::request(op, json!({ "path": path })))
    }

    /// A refusal ends the open.
    fn refuse(&mut self, asked: &str, message: String) -> Answer {
        self.step = None;
        Answer::Now(self.opened(asked, Err(message)))
    }

    /// One storage reply, for the step it answers.
    fn advance(&mut self, asked: &str, step: Step, outcome: Outcome) -> Answer {
        let reply = storage::response(outcome);
        match step {
            Step::Stat => {
                let stat = match reply {
                    Ok(stat) => stat,
                    Err(message) => return self.refuse(asked, message),
                };
                if stat["isDirectory"] == Json::Bool(true) {
                    return self.request(asked, Step::Readme, "fs.readdir", asked);
                }
                let size = stat["size"].as_u64().unwrap_or(0);
                if size > LIMIT {
                    return self.refuse(asked, too_large(asked, size));
                }
                let file = asked.to_owned();
                self.request(
                    asked,
                    Step::Read { file: file.clone() },
                    "fs.readFile",
                    &file,
                )
            }
            Step::Readme => {
                let names = match reply {
                    Ok(names) => names,
                    Err(message) => return self.refuse(asked, message),
                };
                let readme = ["README.md", "readme.md", "index.md", "README.markdown"]
                    .into_iter()
                    .find(|n| names.as_array().is_some_and(|a| a.iter().any(|m| m == n)));
                let Some(readme) = readme else {
                    let message = format!("{} is a folder with no README.md in it", name_of(asked));
                    return self.refuse(asked, message);
                };
                let file = format!("{}/{readme}", asked.trim_end_matches('/'));
                self.request(
                    asked,
                    Step::Read { file: file.clone() },
                    "fs.readFile",
                    &file,
                )
            }
            Step::Read { file } => {
                let bytes = match reply.and_then(|r| {
                    r["base64"]
                        .as_str()
                        .and_then(exact_data::envelope::unbase64)
                        .ok_or_else(|| format!("{} was not read", name_of(&file)))
                }) {
                    Ok(bytes) => bytes,
                    Err(message) => return self.refuse(asked, message),
                };
                if bytes.len() as u64 > LIMIT {
                    return self.refuse(asked, too_large(&file, bytes.len() as u64));
                }
                // Text, not bytes: a file that is not UTF-8 is not this
                // reader's, and saying so is better than showing
                // replacement characters.
                let Ok(text) = String::from_utf8(bytes) else {
                    return self.refuse(asked, format!("{} is not UTF-8 text", name_of(&file)));
                };
                match folder_of(&file) {
                    Some(folder) => {
                        let folder = folder.to_owned();
                        self.request(asked, Step::Siblings { file, text }, "fs.readdir", &folder)
                    }
                    None => self.finish(asked, file, text, Value::list(Vec::new())),
                }
            }
            // A folder that will not list is a document without a list.
            Step::Siblings { file, text } => {
                let siblings = match reply {
                    Ok(names) => siblings(&file, &names),
                    Err(_) => Value::list(Vec::new()),
                };
                self.finish(asked, file, text, siblings)
            }
        }
    }

    /// The text is in hand: parse it on the host's worker, or here where
    /// there is none (a browser).
    fn finish(&mut self, asked: &str, file: String, text: String, siblings: Value) -> Answer {
        self.step = None;
        self.beside = siblings;
        if cfg!(target_arch = "wasm32") {
            let result = parse_text(&file, &text, &|| false);
            return Answer::Now(self.opened(asked, result));
        }
        self.inflight
            .insert(self.next, (asked.to_owned(), file, text));
        Answer::Later(Request::continuation(self.next))
    }
}

fn too_large(path: &str, size: u64) -> String {
    format!(
        "{} is {} MB — larger than this reader opens",
        name_of(path),
        size / (1024 * 1024)
    )
}

/// The folder a document path's file is in, when that folder is one the
/// person chose (`doc:/<n>/<folder>/…`); `None` for a file chosen alone,
/// whose handle holds nothing else.
fn folder_of(file: &str) -> Option<&str> {
    let (folder, _) = file.rsplit_once('/')?;
    let inside = folder.strip_prefix("doc:/")?;
    (inside.split('/').count() >= 2).then_some(folder)
}

/// The Markdown files beside this one, in name order, the open one marked —
/// the only navigation a one-document reader needs, and the reason opening
/// a file in a folder of notes is not a dead end.
fn siblings(file: &str, names: &Json) -> Value {
    let Some(folder) = folder_of(file) else {
        return Value::list(Vec::new());
    };
    let mut found: Vec<&str> = names
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .filter(|n| n.ends_with(".md") || n.ends_with(".markdown"))
        .collect();
    found.sort();
    Value::list(
        found
            .iter()
            .take(500)
            .enumerate()
            .map(|(i, name)| {
                let path = format!("{folder}/{name}");
                Value::record(vec![
                    Value::str(&i.to_string()),
                    Value::str(name),
                    Value::str(&path),
                    Value::Bool(path == file),
                ])
            })
            .collect(),
    )
}

/// What to put in the window for this path: its last name.
pub fn name_of(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// Parse a file's text, its links resolved beside it; `cancel` stops a
/// scan a newer open superseded.
pub fn parse_text(file: &str, text: &str, cancel: &dyn Fn() -> bool) -> Opened {
    if cancel() {
        return Err("file open superseded".into());
    }
    let base = file.rsplit_once('/').map_or("", |(folder, _)| folder);
    let doc =
        markdown_parse::parse_cancellable(text, &|target: &str| resolve(base, target), cancel)
            .ok_or_else(|| "file open superseded".to_string())?;
    Ok((file.to_owned(), doc))
}

/// A link target as the app should act on it: a page stays a URL, an anchor
/// stays an anchor, and a relative path becomes the document path it names
/// beside this one — resolved here, because nothing downstream knows where
/// the document came from. Resolution is lexical: a link to a file that
/// does not exist yet still comes out as the path it names, so the refusal
/// can say so, and a link out of the chosen folder leaves the `doc:`
/// namespace and is refused on open.
pub fn resolve(base: &str, target: &str) -> String {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return target.to_string();
    }
    let without_anchor = target.split('#').next().unwrap_or(target);
    if without_anchor.starts_with('/') {
        return without_anchor.to_string();
    }
    let (scheme, rest) = match base.strip_prefix("doc:/") {
        Some(rest) => ("doc:/", rest),
        None => ("", base),
    };
    let mut parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    for part in without_anchor.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("{scheme}{}", parts.join("/"))
}

impl DataSource for Markdown {
    fn app_id(&self) -> &str {
        "com.exact.markdown"
    }

    fn grants(&self) -> &str {
        GRANTS
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
        let Some(asked) = args.first().and_then(Value::as_str) else {
            return Err(DataError::BadArguments("open(path)".into()));
        };
        let asked = asked.to_string();
        self.next += 1;
        self.generation.store(self.next, Ordering::Release);
        self.inflight.clear();
        self.step = None;
        if let Ok(mut done) = self.done.lock() {
            done.clear();
        }
        if asked.is_empty() {
            return Ok(Answer::Now(self.welcome()));
        }
        if !asked.starts_with("doc:/") {
            let message = format!("{asked} is not a document you opened");
            return Ok(Answer::Now(self.opened(&asked, Err(message))));
        }
        Ok(self.request(&asked, Step::Stat, "fs.stat", &asked))
    }

    fn continuation(&mut self, token: u64) -> Option<Box<dyn FnOnce() -> Outcome + Send>> {
        let (asked, file, text) = self.inflight.remove(&token)?;
        let done = Arc::clone(&self.done);
        let generation = Arc::clone(&self.generation);
        Some(Box::new(move || {
            let cancel = || generation.load(Ordering::Acquire) != token;
            let result = parse_text(&file, &text, &cancel);
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
        let asked = args
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        // A storage reply to the open in progress takes its next step.
        if let Some((generation, open, step)) = self.step.take() {
            if generation == self.next && open == asked {
                return Ok(self.advance(&asked, step, outcome));
            }
            self.step = Some((generation, open, step));
        }
        let result = self
            .done
            .lock()
            .ok()
            .and_then(|mut d| d.remove(&asked))
            .filter(|(generation, _)| *generation == self.next)
            .map(|(_, result)| result);
        let result = match (result, outcome) {
            (Some(result), _) => result,
            (None, Outcome::Failed { kind, message }) => Err(match kind {
                FailureKind::Unsupported => "this surface cannot open local files".to_string(),
                _ => message,
            }),
            (
                None,
                Outcome::Response(_)
                | Outcome::Storage(_)
                | Outcome::Surface(_)
                | Outcome::Message(_),
            ) => Err(format!("{asked} was not read")),
        };
        Ok(Answer::Now(self.opened(&asked, result)))
    }
}

#[cfg(test)]
mod tests;
