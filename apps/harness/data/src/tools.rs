//! The agent's tools: their JSON-schema definitions, a call's title and
//! approval preview, and running one. Output shown in the transcript is
//! cleaned of control sequences and has its tabs expanded.

use crate::highlight::expand_tabs;
use crate::state::{Line, Run};
use serde_json::{json, Value as Json};
use std::collections::HashSet;
use std::io::{BufRead, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Lines of output a tool entry shows; the rest are counted in `more`.
pub const SHOWN: usize = 10;
/// The most a tool result sends back to the model, in bytes.
const RESULT_CAP: usize = 100_000;
/// A command's time limit.
const BASH_TIMEOUT: Duration = Duration::from_secs(120);

/// Every tool's definition: name, description, JSON-schema parameters.
pub fn definitions() -> Vec<(&'static str, &'static str, Json)> {
    vec![
        (
            "read_file",
            "Read a text file. Returns numbered lines. `offset` is the first line (1-based), `limit` how many.",
            json!({"type":"object","properties":{
                "path":{"type":"string","description":"File path, relative to the working directory"},
                "offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}),
        ),
        (
            "list_dir",
            "List a directory's entries; directories end in '/'.",
            json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        ),
        (
            "grep",
            "Search files under a path (default '.') for a plain substring; skips .git, target and node_modules.",
            json!({"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"]}),
        ),
        (
            "bash",
            "Run a shell command (sh -c) in the working directory; stdout and stderr are returned. Times out after 120 s.",
            json!({"type":"object","properties":{"command":{"type":"string"}},"required":["command"]}),
        ),
        (
            "write_file",
            "Create or overwrite a file with the given content.",
            json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"]}),
        ),
        (
            "edit_file",
            "Replace the one occurrence of `old` in a file with `new`.",
            json!({"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"]}),
        ),
    ]
}

fn arg<'a>(input: &'a Json, key: &str) -> &'a str {
    input[key].as_str().unwrap_or("")
}

fn short(text: &str, n: usize) -> String {
    let one = text.lines().next().unwrap_or("");
    if one.chars().count() > n || text.lines().nth(1).is_some() {
        let mut s: String = one.chars().take(n).collect();
        s.push('…');
        s
    } else {
        one.to_string()
    }
}

/// The call as the transcript titles it: `Bash(ls -la)`.
pub fn title(name: &str, input: &Json) -> String {
    match name {
        "read_file" => format!("Read({})", arg(input, "path")),
        "list_dir" => format!("List({})", arg(input, "path")),
        "grep" => format!("Grep({:?})", arg(input, "pattern")),
        "bash" => format!("Bash({})", short(arg(input, "command"), 60)),
        "write_file" => format!("Write({})", arg(input, "path")),
        "edit_file" => format!("Edit({})", arg(input, "path")),
        other => format!("{other}({})", short(&input.to_string(), 40)),
    }
}

/// A `file://` link to the file or directory a call names, for a title a
/// terminal can open (Cmd-click), or empty for a call that names none.
pub fn file_link(name: &str, input: &Json) -> String {
    if !matches!(name, "read_file" | "list_dir" | "write_file" | "edit_file") {
        return String::new();
    }
    let path = std::path::Path::new(arg(input, "path"));
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut url = String::from("file://");
    for c in full.to_string_lossy().chars() {
        match c {
            ' ' => url.push_str("%20"),
            '%' => url.push_str("%25"),
            c => url.push(c),
        }
    }
    url
}

/// Whether a call waits for the person.
pub fn needs_approval(name: &str) -> bool {
    matches!(name, "bash" | "write_file" | "edit_file")
}

fn resolve(path: &str) -> PathBuf {
    let p = Path::new(if path.is_empty() { "." } else { path });
    if p.is_absolute() {
        return p.to_path_buf();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return Path::new(&home).join(rest);
        }
    }
    std::env::current_dir().unwrap_or_default().join(p)
}

/// The edit a write or edit call would make: the file's text before and after.
pub struct Prepared {
    path: PathBuf,
    target: PathBuf,
    /// The previewed file's (device, inode), so a file swapped in with the
    /// same bytes is refused, not written.
    identity: Option<(u64, u64)>,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
}

#[cfg(unix)]
fn identity(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
fn identity(_: &std::fs::Metadata) -> Option<(u64, u64)> {
    None
}

fn target(path: &Path) -> Result<PathBuf, String> {
    if path.exists() {
        return path.canonicalize().map_err(|e| e.to_string());
    }
    let parent = path.parent().ok_or("file has no parent")?;
    if parent == path {
        return Err("file has no parent".into());
    }
    Ok(target(parent)?.join(path.file_name().ok_or("file has no name")?))
}

fn contents(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::symlink_metadata(path) {
                Ok(_) => Err(format!("cannot read {}: {e}", path.display())),
                Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(other) => Err(format!("cannot inspect {}: {other}", path.display())),
            }
        }
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

fn prepare(name: &str, input: &Json) -> Result<Prepared, String> {
    let path = resolve(arg(input, "path"));
    let target = target(&path)?;
    let before = contents(&path)?;
    let after = match name {
        "write_file" => arg(input, "content").as_bytes().to_vec(),
        "edit_file" => {
            let bytes = before
                .as_ref()
                .ok_or_else(|| format!("cannot read {}: file is absent", path.display()))?;
            let text = std::str::from_utf8(bytes)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let old = arg(input, "old");
            if old.is_empty() {
                return Err("`old` is empty".into());
            }
            match text.matches(old).count() {
                0 => Err(format!("`old` not found in {}", path.display())),
                1 => Ok(text.replacen(old, arg(input, "new"), 1).into_bytes()),
                n => Err(format!("`old` occurs {n} times; give more context")),
            }?
        }
        _ => return Err(format!("{name} makes no edit")),
    };
    let identity = std::fs::metadata(&target).ok().and_then(|m| identity(&m));
    Ok(Prepared {
        path,
        target,
        identity,
        before,
        after,
    })
}

/// What an approval shows: its summary and lines (the command, or the diff).
pub fn preview(name: &str, input: &Json) -> (String, Vec<Line>, Option<Result<Prepared, String>>) {
    match name {
        "bash" => {
            let command = arg(input, "command");
            let (summary, lines) = (
                format!("Run `{}`?", short(command, 60)),
                command
                    .lines()
                    .map(|l| Line::plain(expand_tabs(l, 8)))
                    .collect(),
            );
            (summary, lines, None)
        }
        "write_file" | "edit_file" => {
            let verb = if name == "write_file" {
                "Write"
            } else {
                "Edit"
            };
            let summary = format!("{verb} {}?", arg(input, "path"));
            match prepare(name, input) {
                Ok(edit) => {
                    let before =
                        String::from_utf8_lossy(edit.before.as_deref().unwrap_or_default());
                    let after = String::from_utf8_lossy(&edit.after);
                    let lines = if before == after && edit.before.as_ref() != Some(&edit.after) {
                        vec![Line::plain(
                            "(the bytes change, though the text reads the same: the file is not UTF-8)",
                        )]
                    } else {
                        diff(&before, &after)
                    };
                    (summary, lines, Some(Ok(edit)))
                }
                Err(e) => (summary, vec![Line::plain(&e)], Some(Err(e))),
            }
        }
        other => (format!("Run {other}?"), vec![], None),
    }
}

/// A finished call: what the model is told, what the entry shows.
pub struct Done {
    pub text: String,
    pub lines: Vec<Line>,
    pub is_error: bool,
}

impl Done {
    pub fn error(text: impl Into<String>) -> Done {
        Done::err(text)
    }

    fn ok(text: String) -> Done {
        Done {
            lines: output_lines(&text),
            text: cap(text),
            is_error: false,
        }
    }

    fn err(text: impl Into<String>) -> Done {
        let text = text.into();
        Done {
            lines: output_lines(&text),
            text,
            is_error: true,
        }
    }
}

fn cap(mut text: String) -> String {
    if text.len() > RESULT_CAP {
        let mut end = RESULT_CAP;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[output truncated]");
    }
    text
}

/// One line of program output as transcript text: tabs expanded, ESC
/// shown as "␛", every other control character dropped.
pub fn clean_line(line: &str) -> String {
    let line = expand_tabs(line, 8);
    line.chars()
        .filter_map(|c| match c {
            '\u{1b}' => Some('␛'),
            c if c.is_control() => None,
            c => Some(c),
        })
        .collect()
}

/// Output as plain lines.
pub fn output_lines(text: &str) -> Vec<Line> {
    text.trim_end_matches('\n')
        .split('\n')
        .map(|l| Line::plain(clean_line(l)))
        .collect()
}

/// Run a call. `cancel` stops a running command.
pub fn run(name: &str, input: &Json, cancel: &AtomicBool) -> Done {
    if let Some(why) = crate::sse::invalid_arguments(input) {
        return Done::err(format!("invalid JSON arguments: {why}"));
    }
    match name {
        "read_file" => read_file(input, cancel),
        "list_dir" => list_dir(arg(input, "path")),
        "grep" => grep(arg(input, "pattern"), arg(input, "path"), cancel),
        "bash" => bash(arg(input, "command"), cancel),
        "write_file" | "edit_file" => match prepare(name, input) {
            Ok(edit) => run_prepared(name, &edit, cancel),
            Err(e) => Done::err(e),
        },
        other => Done::err(format!("no tool named {other}")),
    }
}

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// Apply exactly the edit shown for approval, if its target and bytes still match.
pub fn run_prepared(name: &str, edit: &Prepared, cancel: &AtomicBool) -> Done {
    if cancel.load(Ordering::SeqCst) {
        return Done::err("cancelled");
    }
    let changed = "file changed since approval; ask again";
    if target(&edit.path).ok().as_ref() != Some(&edit.target)
        || contents(&edit.path).ok().as_ref() != Some(&edit.before)
    {
        return Done::err(changed);
    }
    let parent = edit.target.parent().expect("file target has a parent");
    if let Err(e) = std::fs::create_dir_all(parent) {
        return Done::err(format!("cannot create {}: {e}", parent.display()));
    }
    if target(&edit.path).ok().as_ref() != Some(&edit.target)
        || contents(&edit.path).ok().as_ref() != Some(&edit.before)
    {
        return Done::err(changed);
    }
    if edit.before.is_some() {
        let applied = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&edit.target)?;
            let mut current = Vec::new();
            file.read_to_end(&mut current)?;
            if Some(&current) != edit.before.as_ref()
                || target(&edit.path).ok().as_ref() != Some(&edit.target)
                || identity(&file.metadata()?) != edit.identity
            {
                return Err(std::io::Error::other(changed));
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("cancelled"));
            }
            file.rewind()?;
            file.write_all(&edit.after)?;
            file.set_len(edit.after.len() as u64)?;
            file.sync_all()
        })();
        if let Err(e) = applied {
            return Done::err(format!("cannot write {}: {e}", edit.path.display()));
        }
        return completed_edit(name, edit);
    }
    let mut temp = None;
    for _ in 0..16 {
        let path = parent.join(format!(
            ".exact-write-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                temp = Some((path, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Done::err(format!("cannot write {}: {e}", edit.path.display())),
        }
    }
    let Some((path, mut file)) = temp else {
        return Done::err("cannot create temporary file");
    };
    let written = (|| -> std::io::Result<()> {
        file.write_all(&edit.after)?;
        file.sync_all()?;
        if cancel.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("cancelled"));
        }
        if target(&edit.path).ok().as_ref() != Some(&edit.target)
            || contents(&edit.path).ok().as_ref() != Some(&edit.before)
        {
            return Err(std::io::Error::other(changed));
        }
        publish_new_file(&path, &edit.target, &edit.after)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(path);
        return Done::err(format!("cannot write {}: {e}", edit.path.display()));
    }
    completed_edit(name, edit)
}

fn publish_new_file(temp: &Path, target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    publish_new_file_with(temp, target, bytes, |from, to| std::fs::hard_link(from, to))
}

fn publish_new_file_with(
    temp: &Path,
    target: &Path,
    bytes: &[u8],
    link: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    match link(temp, target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::Unsupported => {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                let _ = std::fs::remove_file(target);
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    }
    std::fs::remove_file(temp)
}

fn completed_edit(name: &str, edit: &Prepared) -> Done {
    let before = String::from_utf8_lossy(edit.before.as_deref().unwrap_or_default());
    let after = String::from_utf8_lossy(&edit.after);
    Done {
        text: format!(
            "{} {}",
            if name == "write_file" {
                "Wrote"
            } else {
                "Edited"
            },
            edit.path.display()
        ),
        lines: diff(&before, &after),
        is_error: false,
    }
}

fn read_file(input: &Json, cancel: &AtomicBool) -> Done {
    if cancel.load(Ordering::SeqCst) {
        return Done::err("cancelled");
    }
    let path = resolve(arg(input, "path"));
    let mut file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => return Done::err(format!("cannot read {}: {e}", path.display())),
    };
    let mut prefix = Vec::new();
    if let Err(e) = Read::by_ref(&mut file).take(8000).read_to_end(&mut prefix) {
        return Done::err(format!("cannot read {}: {e}", path.display()));
    }
    if prefix.contains(&0) {
        return Done::err(format!("{} is a binary file", path.display()));
    }
    let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = input["limit"].as_u64().unwrap_or(2000) as usize;
    let mut reader = std::io::BufReader::new(std::io::Cursor::new(prefix).chain(file));
    let mut model = String::new();
    let mut shown = Vec::new();
    let mut transcript_bytes = 0;
    let mut line = Vec::new();
    let mut line_no = 1usize;
    let mut line_started = false;
    let mut line_truncated = false;
    let mut output_truncated = false;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Done::err("cancelled");
        }
        let chunk = match reader.fill_buf() {
            Ok(b) => b,
            Err(e) => return Done::err(format!("cannot read {}: {e}", path.display())),
        };
        if chunk.is_empty() {
            break;
        }
        let size = chunk.len();
        for part in chunk.split_inclusive(|&b| b == b'\n') {
            let ended = part.last() == Some(&b'\n');
            let bytes = if ended { &part[..part.len() - 1] } else { part };
            line_started = true;
            if line_no >= offset && line_no - offset < limit {
                let keep = bytes.len().min(8000usize.saturating_sub(line.len()));
                line.extend_from_slice(&bytes[..keep]);
                line_truncated |= keep < bytes.len();
            }
            if ended {
                if line_no >= offset && line_no - offset < limit {
                    let text = String::from_utf8_lossy(line.strip_suffix(b"\r").unwrap_or(&line));
                    let marker = if line_truncated {
                        " [line truncated]"
                    } else {
                        ""
                    };
                    let numbered = format!("{:>6}\t{text}{marker}\n", line_no);
                    let display = clean_line(&format!("{text}{marker}"));
                    if model.len() + numbered.len() > RESULT_CAP
                        || transcript_bytes + display.len() > RESULT_CAP
                    {
                        output_truncated = true;
                        break;
                    }
                    transcript_bytes += display.len();
                    model.push_str(&numbered);
                    shown.push(Line::plain(display));
                }
                line.clear();
                line_truncated = false;
                line_started = false;
                line_no += 1;
            }
        }
        reader.consume(size);
        if output_truncated || line_no >= offset.saturating_add(limit) {
            break;
        }
    }
    if line_started && line_no >= offset && line_no - offset < limit && !output_truncated {
        let text = String::from_utf8_lossy(&line);
        let marker = if line_truncated {
            " [line truncated]"
        } else {
            ""
        };
        let numbered = format!("{:>6}\t{text}{marker}\n", line_no);
        let display = clean_line(&format!("{text}{marker}"));
        if model.len() + numbered.len() <= RESULT_CAP
            && transcript_bytes + display.len() <= RESULT_CAP
        {
            model.push_str(&numbered);
            shown.push(Line::plain(display));
        } else {
            output_truncated = true;
        }
    }
    if output_truncated {
        model.push_str("[output truncated]");
        shown.push(Line::plain("[output truncated]"));
    }
    if shown.is_empty() {
        shown.push(Line::plain("(empty)"));
    }
    Done {
        text: model,
        lines: shown,
        is_error: false,
    }
}

fn list_dir(path: &str) -> Done {
    let dir = resolve(path);
    let read = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(e) => return Done::err(format!("cannot list {}: {e}", dir.display())),
    };
    let mut names: Vec<String> = read
        .filter_map(|e| e.ok())
        .map(|e| {
            let mut n = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                n.push('/');
            }
            n
        })
        .collect();
    names.sort();
    if names.is_empty() {
        return Done::ok("(empty directory)".into());
    }
    Done::ok(names.join("\n"))
}

const SKIP: [&str; 3] = [".git", "target", "node_modules"];
const GREP_CAP: usize = 200;
const GREP_WORK_CAP: usize = 20_000;
const GREP_BYTES_CAP: u64 = 32_000_000;

fn read_grep_bytes(
    reader: &mut impl Read,
    limit: u64,
    bytes_read: &mut u64,
    cancel: &AtomicBool,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut reader = reader.take(limit);
    let mut chunk = [0u8; 8192];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "cancelled",
            ));
        }
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        *bytes_read += n as u64;
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(bytes)
}

fn grep(pattern: &str, path: &str, cancel: &AtomicBool) -> Done {
    grep_with_cap(pattern, path, cancel, GREP_WORK_CAP)
}

fn grep_with_cap(pattern: &str, path: &str, cancel: &AtomicBool, work_cap: usize) -> Done {
    if pattern.is_empty() {
        return Done::err("empty pattern");
    }
    if cancel.load(Ordering::SeqCst) {
        return Done::err("cancelled");
    }
    let root = resolve(if path.is_empty() { "." } else { path });
    let base = std::env::current_dir().unwrap_or_default();
    let mut hits = Vec::new();
    let mut stack = vec![root];
    let mut discovered = 1;
    let mut directories = HashSet::new();
    let mut bytes_read = 0u64;
    let mut partial = false;
    while let Some(p) = stack.pop() {
        if cancel.load(Ordering::SeqCst) {
            return Done::err("cancelled");
        }
        if hits.len() >= GREP_CAP {
            break;
        }
        let Ok(meta) = p.metadata() else {
            continue;
        };
        if meta.is_dir() {
            #[cfg(unix)]
            let identity = {
                use std::os::unix::fs::MetadataExt;
                (meta.dev(), meta.ino())
            };
            #[cfg(not(unix))]
            let identity = p.canonicalize().unwrap_or_else(|_| p.clone());
            if !directories.insert(identity) {
                continue;
            }
            let Ok(read) = std::fs::read_dir(&p) else {
                continue;
            };
            let mut children = Vec::new();
            for entry in read {
                if cancel.load(Ordering::SeqCst) {
                    return Done::err("cancelled");
                }
                let Ok(e) = entry else { continue };
                if SKIP.contains(&e.file_name().to_string_lossy().as_ref()) {
                    continue;
                }
                if discovered >= work_cap {
                    partial = true;
                    break;
                }
                discovered += 1;
                children.push(e.path());
            }
            children.sort();
            children.reverse();
            stack.extend(children);
            continue;
        }
        if !meta.is_file() || meta.len() > 2_000_000 {
            continue;
        }
        if bytes_read >= GREP_BYTES_CAP {
            partial = true;
            break;
        }
        let Ok(mut file) = std::fs::File::open(&p) else {
            continue;
        };
        let allowance = (GREP_BYTES_CAP - bytes_read).min(2_000_000);
        // One byte past the allowance tells a file cut short from one read whole.
        let Ok(mut bytes) = read_grep_bytes(&mut file, allowance + 1, &mut bytes_read, cancel)
        else {
            if cancel.load(Ordering::SeqCst) {
                return Done::err("cancelled");
            }
            continue;
        };
        if bytes.len() as u64 > allowance {
            bytes.truncate(allowance as usize);
            partial = true;
        }
        if bytes.iter().take(8000).any(|&b| b == 0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let shown = p.strip_prefix(&base).unwrap_or(&p).display().to_string();
        for (n, line) in text.lines().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Done::err("cancelled");
            }
            if line.contains(pattern) {
                let excerpt: String = line.trim().chars().take(8000).collect();
                hits.push(format!("{shown}:{}: {excerpt}", n + 1));
                if hits.len() >= GREP_CAP {
                    hits.push(format!("[stopped at {GREP_CAP} matches]"));
                    break;
                }
            }
        }
        if bytes_read >= GREP_BYTES_CAP {
            break;
        }
    }
    if partial {
        hits.insert(
            0,
            format!(
                "[search stopped after {discovered} entries or {bytes_read} bytes; partial result]"
            ),
        );
    }
    if hits.is_empty() {
        return Done::ok("no matches".into());
    }
    Done::ok(hits.join("\n"))
}

fn bash(command: &str, cancel: &AtomicBool) -> Done {
    if command.trim().is_empty() {
        return Done::err("empty command");
    }
    let ran = match crate::shell::run(command, cancel, BASH_TIMEOUT, &mut |_| {}) {
        Ok(ran) => ran,
        Err(e) => return Done::err(e),
    };
    let mut text = ran.text;
    if let Some(why) = ran.killed {
        let why = if why == "timed out" {
            format!("timed out after {} s", BASH_TIMEOUT.as_secs())
        } else {
            why.to_string()
        };
        text.push_str(&format!("\n[{why}]"));
        return Done::err(text);
    }
    if text.trim().is_empty() {
        text = "(no output)".into();
    }
    match ran.code {
        Some(0) => Done::ok(text),
        Some(c) => {
            let mut d = Done::ok(format!("{text}\n[exit {c}]"));
            d.is_error = true;
            d
        }
        None => Done::err(format!("{text}\n[killed by a signal]")),
    }
}

/// A removed line's background.
pub const DEL_BG: &str = "#4b1d22";
/// An added line's background.
pub const ADD_BG: &str = "#1d3b26";

/// The line-level edit script between two texts: (old index, new index)
/// pairs that match, by longest common subsequence after trimming the
/// common ends. A middle too large for the table is all removed, all added.
fn matches(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf]
    {
        suf += 1;
    }
    let mut out: Vec<(usize, usize)> = (0..pre).map(|i| (i, i)).collect();
    let (ma, mb) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if !ma.is_empty() && !mb.is_empty() && ma.len() * mb.len() <= 4_000_000 {
        let (n, m) = (ma.len(), mb.len());
        let mut t = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * (m + 1) + j] = if ma[i] == mb[j] {
                    t[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    t[(i + 1) * (m + 1) + j].max(t[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if ma[i] == mb[j] {
                out.push((pre + i, pre + j));
                i += 1;
                j += 1;
            } else if t[(i + 1) * (m + 1) + j] >= t[i * (m + 1) + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    out.extend((0..suf).map(|k| (a.len() - suf + k, b.len() - suf + k)));
    out
}

fn diff_line(num: usize, sign: char, text: &str) -> Line {
    let mut line = Line::default();
    let (bg, fg) = match sign {
        '-' => (DEL_BG, "#e06c75"),
        '+' => (ADD_BG, "#98c379"),
        _ => ("", ""),
    };
    line.push(Run::dim(format!("{num:>4} ")));
    line.push(Run {
        text: format!("{sign} "),
        fg: fg.into(),
        bg: bg.into(),
        ..Run::default()
    });
    line.push(Run {
        text: clean_line(text),
        bg: bg.into(),
        ..Run::default()
    });
    line
}

/// A coloured unified diff of `before` → `after`: line numbers, removals on
/// red, additions on green, three lines of context, `⋮` between hunks.
pub fn diff(before: &str, after: &str) -> Vec<Line> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let pairs = matches(&a, &b);
    // The script: ' ' (old, new), '-' old, '+' new.
    let mut script: Vec<(char, usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pairs.iter().chain(std::iter::once(&(a.len(), b.len()))) {
        while i < pi {
            script.push(('-', i, j));
            i += 1;
        }
        while j < pj {
            script.push(('+', i, j));
            j += 1;
        }
        if pi < a.len() && pj < b.len() {
            script.push((' ', i, j));
            i += 1;
            j += 1;
        }
    }
    let changed: Vec<usize> = (0..script.len()).filter(|&k| script[k].0 != ' ').collect();
    if changed.is_empty() {
        return vec![Line {
            runs: vec![Run::dim("(no changes)")],
        }];
    }
    let mut keep = vec![false; script.len()];
    for &k in &changed {
        for slot in keep
            .iter_mut()
            .take((k + 4).min(script.len()))
            .skip(k.saturating_sub(3))
        {
            *slot = true;
        }
    }
    let mut out = Vec::new();
    let mut last: Option<usize> = None;
    for (k, &(sign, oi, nj)) in script.iter().enumerate() {
        if !keep[k] {
            continue;
        }
        if last.is_some_and(|l| l + 1 != k) {
            out.push(Line {
                runs: vec![Run::dim("   ⋮")],
            });
        }
        last = Some(k);
        out.push(match sign {
            '-' => diff_line(oi + 1, '-', a[oi]),
            '+' => diff_line(nj + 1, '+', b[nj]),
            _ => diff_line(nj + 1, ' ', b[nj]),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "exact-harness-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn approved_write_rejects_changed_file() {
        let dir = test_dir("approval");
        let path = dir.join("note.txt");
        std::fs::write(&path, "approved-before\n").unwrap();
        let input = json!({"path": path, "content": "approved-after\n"});
        let (_, _, edit) = preview("write_file", &input);
        std::fs::write(&path, "concurrent-edit\n").unwrap();
        let done = run_prepared(
            "write_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(done.is_error, "approved a stale diff");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "concurrent-edit\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn approved_edits_apply_only_to_the_previewed_target() {
        let dir = test_dir("approved-target");
        let path = dir.join("note.txt");
        std::fs::write(&path, "old\n").unwrap();
        let input = json!({"path":path, "old":"old", "new":"new"});
        let (_, _, edit) = preview("edit_file", &input);
        let done = run_prepared(
            "edit_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(!done.is_error, "{}", done.text);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");

        let fresh = dir.join("fresh.txt");
        let input = json!({"path":fresh, "content":"approved\n"});
        let (_, _, edit) = preview("write_file", &input);
        std::fs::write(&fresh, "appeared\n").unwrap();
        let done = run_prepared(
            "write_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(done.is_error);
        assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "appeared\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_file_publication_refuses_an_existing_destination() {
        let dir = test_dir("publish-collision");
        let temp = dir.join("temp.txt");
        let target = dir.join("target.txt");
        std::fs::write(&temp, "approved\n").unwrap();
        std::fs::write(&target, "concurrent\n").unwrap();
        let result = publish_new_file(&temp, &target, b"approved\n");
        assert!(result.is_err(), "publication replaced a concurrent file");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "concurrent\n");
        std::fs::remove_file(&target).unwrap();
        publish_new_file(&temp, &target, b"approved\n").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "approved\n");
        assert!(!temp.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_file_publication_falls_back_when_links_are_unsupported() {
        let dir = test_dir("publish-no-links");
        let temp = dir.join("temp.txt");
        let target = dir.join("target.txt");
        std::fs::write(&temp, "approved\n").unwrap();
        let unsupported = |_: &Path, _: &Path| Err(std::io::ErrorKind::Unsupported.into());
        publish_new_file_with(&temp, &target, b"approved\n", unsupported).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "approved\n");
        assert!(!temp.exists());
        std::fs::write(&temp, "approved again\n").unwrap();
        let unsupported = |_: &Path, _: &Path| Err(std::io::ErrorKind::Unsupported.into());
        assert!(publish_new_file_with(&temp, &target, b"approved again\n", unsupported).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "approved\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn approved_write_rejects_a_redirected_link() {
        let dir = test_dir("approval-link");
        let first = dir.join("first.txt");
        let second = dir.join("second.txt");
        let link = dir.join("link.txt");
        std::fs::write(&first, "same\n").unwrap();
        std::fs::write(&second, "same\n").unwrap();
        std::os::unix::fs::symlink(&first, &link).unwrap();
        let input = json!({"path":link, "content":"changed\n"});
        let (_, _, edit) = preview("write_file", &input);
        std::fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&second, &link).unwrap();
        let done = run_prepared(
            "write_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(done.is_error);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "same\n");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "same\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn approved_edit_preserves_existing_file_identity() {
        use std::os::unix::fs::MetadataExt;
        let dir = test_dir("metadata");
        let path = dir.join("note.txt");
        std::fs::write(&path, "old\n").unwrap();
        let before = std::fs::metadata(&path).unwrap().ino();
        let (_, _, edit) = preview("edit_file", &json!({"path":path, "old":"old", "new":"new"}));
        let done = run_prepared(
            "edit_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(!done.is_error, "{}", done.text);
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), before);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn approved_edit_refuses_a_swapped_file_with_the_same_bytes() {
        let dir = test_dir("identity-swap");
        let path = dir.join("a.txt");
        let other = dir.join("b.txt");
        std::fs::write(&path, "same\n").unwrap();
        std::fs::write(&other, "same\n").unwrap();
        let (_, _, edit) = preview(
            "edit_file",
            &json!({"path":path, "old":"same", "new":"new"}),
        );
        std::fs::remove_file(&path).unwrap();
        std::fs::hard_link(&other, &path).unwrap();
        let done = run_prepared(
            "edit_file",
            &edit.unwrap().unwrap(),
            &AtomicBool::new(false),
        );
        assert!(done.is_error, "{}", done.text);
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "same\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preview_shows_a_byte_change_that_reads_as_the_same_text() {
        let dir = test_dir("lossy-preview");
        let path = dir.join("note.txt");
        std::fs::write(&path, [0xFFu8]).unwrap();
        let (_, lines, _) = preview("write_file", &json!({"path":path, "content":"\u{fffd}"}));
        let shown: Vec<String> = lines.iter().map(|l| l.text()).collect();
        assert!(shown.iter().any(|l| l.contains("not UTF-8")), "{shown:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn grep_reads_a_file_exactly_at_its_allowance_completely() {
        let dir = test_dir("grep-exact-allowance");
        std::fs::write(dir.join("big.txt"), "a".repeat(2_000_000)).unwrap();
        std::fs::write(dir.join("small.txt"), "needle\n").unwrap();
        let done = grep_with_cap(
            "needle",
            dir.to_str().unwrap(),
            &AtomicBool::new(false),
            100,
        );
        assert!(done.text.contains("needle"), "{}", done.text);
        assert!(!done.text.contains("partial result"), "{}", done.text);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn preview_retains_preparation_error_for_execution() {
        let dir = test_dir("prepare-error");
        let path = dir.join("note.txt");
        std::fs::write(&path, "old old\n").unwrap();
        let (_, lines, prepared) =
            preview("edit_file", &json!({"path":path, "old":"old", "new":"new"}));
        assert!(lines[0].text().contains("occurs 2 times"));
        match prepared.expect("preparation error was discarded") {
            Err(error) => assert!(error.contains("occurs 2 times"), "{error}"),
            Ok(_) => panic!("ambiguous edit was prepared"),
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn grep_skips_directory_links_and_obeys_cancellation() {
        let dir = test_dir("grep");
        std::fs::write(dir.join("one.txt"), "needle\n").unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("left")).unwrap();
        std::os::unix::fs::symlink(&dir, dir.join("right")).unwrap();
        let input = json!({"pattern":"needle", "path":dir});
        let cancelled = AtomicBool::new(true);
        let stopped = run("grep", &input, &cancelled);
        assert!(stopped.text.contains("cancelled"));
        cancelled.store(false, Ordering::SeqCst);
        let found = run("grep", &input, &cancelled);
        assert_eq!(found.text.lines().count(), 1, "{}", found.text);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn grep_follows_file_links_and_a_linked_search_root() {
        let dir = test_dir("grep-links");
        let real = dir.join("real.txt");
        let link = dir.join("link.txt");
        std::fs::write(&real, "needle\n").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let found = run(
            "grep",
            &json!({"pattern":"needle", "path":link}),
            &AtomicBool::new(false),
        );
        assert!(found.text.contains("link.txt:1:"), "{}", found.text);
        let linked_dir = dir.join("linked-dir");
        std::os::unix::fs::symlink(&dir, &linked_dir).unwrap();
        let found = run(
            "grep",
            &json!({"pattern":"needle", "path":linked_dir}),
            &AtomicBool::new(false),
        );
        assert!(found.text.contains("needle"), "{}", found.text);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn grep_caps_all_discovered_entries() {
        let dir = test_dir("grep-budget");
        let nested = dir.join("a");
        std::fs::create_dir(&nested).unwrap();
        for n in 0..5 {
            std::fs::write(dir.join(format!("z{n}.txt")), "none\n").unwrap();
            std::fs::write(nested.join(format!("n{n}.txt")), "none\n").unwrap();
        }
        let done = grep_with_cap("needle", dir.to_str().unwrap(), &AtomicBool::new(false), 8);
        assert!(
            done.text.contains("[search stopped after 8 entries"),
            "{}",
            done.text
        );
        let done = grep_with_cap("none", dir.to_str().unwrap(), &AtomicBool::new(false), 8);
        assert!(done.text.contains("z0.txt:1: none"), "{}", done.text);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn grep_partial_notice_survives_result_cap() {
        let dir = test_dir("grep-partial-cap");
        for n in 0..16 {
            std::fs::write(
                dir.join(format!("{n:02}.txt")),
                format!("needle{}\n", "x".repeat(8_000)),
            )
            .unwrap();
        }
        let done = grep_with_cap("needle", dir.to_str().unwrap(), &AtomicBool::new(false), 16);
        assert!(
            done.text.starts_with("[search stopped"),
            "{}",
            done.text.chars().take(100).collect::<String>()
        );
        assert!(done.text.contains("partial result]"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn grep_reads_only_the_remaining_byte_budget() {
        struct CountingReader {
            read: usize,
        }
        impl Read for CountingReader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                let n = buf.len().min(256usize.saturating_sub(self.read));
                self.read += n;
                buf[..n].fill(b'x');
                Ok(n)
            }
        }
        let mut reader = CountingReader { read: 0 };
        let mut charged = 0;
        let bytes =
            read_grep_bytes(&mut reader, 128, &mut charged, &AtomicBool::new(false)).unwrap();
        assert_eq!(bytes.len(), 128);
        assert_eq!(reader.read, 128);
        assert_eq!(charged, 128);
    }

    #[test]
    fn read_file_bounds_a_long_line_and_obeys_cancellation() {
        let dir = test_dir("read");
        let path = dir.join("large.txt");
        std::fs::write(&path, "x".repeat(2_000_000)).unwrap();
        let input = json!({"path":path, "limit":1});
        let cancelled = run("read_file", &input, &AtomicBool::new(true));
        assert!(cancelled.text.contains("cancelled"));
        let done = run("read_file", &input, &AtomicBool::new(false));
        assert!(done.text.len() <= RESULT_CAP + 32);
        assert!(done.lines[0].text().len() <= RESULT_CAP);
        assert!(done.text.contains("truncated"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_file_preserves_offset_and_unicode_and_sniffs_binary() {
        let dir = test_dir("read-lines");
        let path = dir.join("text.txt");
        std::fs::write(&path, "first\r\nαβ\nthird\n").unwrap();
        let done = run(
            "read_file",
            &json!({"path":path,"offset":2,"limit":1}),
            &AtomicBool::new(false),
        );
        assert_eq!(done.text, "     2\tαβ\n");
        assert_eq!(done.lines[0].text(), "αβ");
        std::fs::write(&path, b"a\0b").unwrap();
        let done = run(
            "read_file",
            &json!({"path":path,"limit":1}),
            &AtomicBool::new(false),
        );
        assert!(done.is_error && done.text.contains("binary file"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn read_file_preserves_lone_carriage_return_at_eof() {
        let dir = test_dir("read-cr-eof");
        let path = dir.join("text.txt");
        std::fs::write(&path, b"hello\r\nworld\r").unwrap();
        let done = run("read_file", &json!({"path":path}), &AtomicBool::new(false));
        assert_eq!(done.text, "     1\thello\n     2\tworld\r\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn read_file_accepts_non_seekable_input_after_binary_sniff() {
        let dir = test_dir("read-fifo");
        let path = dir.join("input.fifo");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .open(writer_path)
                .unwrap();
            file.write_all(&vec![b'a'; 8_100]).unwrap();
            file.write_all(b"\nhello\n").unwrap();
        });
        let done = run(
            "read_file",
            &json!({"path":path,"offset":2,"limit":1}),
            &AtomicBool::new(false),
        );
        writer.join().unwrap();
        assert_eq!(done.text, "     2\thello\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn diff_marks_changes_with_context() {
        let before = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let after = "a\nb\nc\nd\nE\nf\ng\nh\ni\nj\nk\n";
        let lines = diff(before, after);
        let text: Vec<String> = lines.iter().map(Line::text).collect();
        assert!(text.contains(&"   5 - e".to_string()), "{text:?}");
        assert!(text.contains(&"   5 + E".to_string()), "{text:?}");
        assert!(text.contains(&"  11 + k".to_string()), "{text:?}");
        assert!(lines.iter().any(|l| l.runs.iter().any(|r| r.bg == DEL_BG)));
    }

    #[test]
    fn output_is_cleaned() {
        let lines = output_lines("a\tb\n\u{1b}[31mred\u{1b}[0m\r\n");
        assert_eq!(lines[0].text(), "a       b");
        assert_eq!(lines[1].text(), "␛[31mred␛[0m");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn bash_captures_both_streams() {
        let d = bash("echo out; echo err 1>&2; exit 3", &AtomicBool::new(false));
        assert!(d.is_error);
        assert!(d.text.contains("out") && d.text.contains("err") && d.text.contains("[exit 3]"));
    }

    #[test]
    fn titles() {
        assert_eq!(title("bash", &json!({"command":"ls -la"})), "Bash(ls -la)");
        assert_eq!(title("grep", &json!({"pattern":"foo"})), "Grep(\"foo\")");
        assert_eq!(
            title("read_file", &json!({"path":"Cargo.toml"})),
            "Read(Cargo.toml)"
        );
    }
}
