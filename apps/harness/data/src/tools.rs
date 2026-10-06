//! The agent's tools: their JSON-schema definitions, a call's title and
//! approval preview, and running one. Output shown in the transcript is
//! cleaned of control sequences and has its tabs expanded.

use crate::highlight::expand_tabs;
use crate::state::{Line, Run};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
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
fn change(name: &str, input: &Json) -> Result<(String, String), String> {
    let path = resolve(arg(input, "path"));
    match name {
        "write_file" => {
            let before = std::fs::read_to_string(&path).unwrap_or_default();
            Ok((before, arg(input, "content").to_string()))
        }
        "edit_file" => {
            let before = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let old = arg(input, "old");
            if old.is_empty() {
                return Err("`old` is empty".into());
            }
            match before.matches(old).count() {
                0 => Err(format!("`old` not found in {}", path.display())),
                1 => {
                    let after = before.replacen(old, arg(input, "new"), 1);
                    Ok((before, after))
                }
                n => Err(format!("`old` occurs {n} times; give more context")),
            }
        }
        _ => Err(format!("{name} makes no edit")),
    }
}

/// What an approval shows: its summary and lines (the command, or the diff).
pub fn preview(name: &str, input: &Json) -> (String, Vec<Line>) {
    match name {
        "bash" => {
            let command = arg(input, "command");
            (
                format!("Run `{}`?", short(command, 60)),
                command
                    .lines()
                    .map(|l| Line::plain(expand_tabs(l, 8)))
                    .collect(),
            )
        }
        "write_file" | "edit_file" => {
            let verb = if name == "write_file" {
                "Write"
            } else {
                "Edit"
            };
            let summary = format!("{verb} {}?", arg(input, "path"));
            match change(name, input) {
                Ok((before, after)) => (summary, diff(&before, &after)),
                Err(e) => (summary, vec![Line::plain(e)]),
            }
        }
        other => (format!("Run {other}?"), vec![]),
    }
}

/// A finished call: what the model is told, what the entry shows.
pub struct Done {
    pub text: String,
    pub lines: Vec<Line>,
    pub is_error: bool,
}

impl Done {
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
        "read_file" => read_file(input),
        "list_dir" => list_dir(arg(input, "path")),
        "grep" => grep(arg(input, "pattern"), arg(input, "path")),
        "bash" => bash(arg(input, "command"), cancel),
        "write_file" | "edit_file" => match change(name, input) {
            Ok((before, after)) => {
                let path = resolve(arg(input, "path"));
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, &after) {
                    Ok(()) => Done {
                        text: format!(
                            "{} {}",
                            if name == "write_file" {
                                "Wrote"
                            } else {
                                "Edited"
                            },
                            path.display()
                        ),
                        lines: diff(&before, &after),
                        is_error: false,
                    },
                    Err(e) => Done::err(format!("cannot write {}: {e}", path.display())),
                }
            }
            Err(e) => Done::err(e),
        },
        other => Done::err(format!("no tool named {other}")),
    }
}

fn read_file(input: &Json) -> Done {
    let path = resolve(arg(input, "path"));
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return Done::err(format!("cannot read {}: {e}", path.display())),
    };
    if bytes.iter().take(8000).any(|&b| b == 0) {
        return Done::err(format!("{} is a binary file", path.display()));
    }
    let text = String::from_utf8_lossy(&bytes);
    let offset = input["offset"].as_u64().unwrap_or(1).max(1) as usize;
    let limit = input["limit"].as_u64().unwrap_or(2000) as usize;
    let mut model = String::new();
    let mut shown = Vec::new();
    for (n, line) in text.lines().enumerate().skip(offset - 1).take(limit) {
        model.push_str(&format!("{:>6}\t{line}\n", n + 1));
        shown.push(line);
    }
    let mut done = Done::ok(model);
    done.lines = shown.iter().map(|l| Line::plain(clean_line(l))).collect();
    if done.lines.is_empty() {
        done.lines.push(Line::plain("(empty)"));
    }
    done
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

fn grep(pattern: &str, path: &str) -> Done {
    if pattern.is_empty() {
        return Done::err("empty pattern");
    }
    let root = resolve(if path.is_empty() { "." } else { path });
    let base = std::env::current_dir().unwrap_or_default();
    let mut hits = Vec::new();
    let mut stack = vec![root];
    while let Some(p) = stack.pop() {
        if hits.len() >= GREP_CAP {
            break;
        }
        if p.is_dir() {
            let Ok(read) = std::fs::read_dir(&p) else {
                continue;
            };
            let mut children: Vec<PathBuf> = read
                .filter_map(|e| e.ok())
                .filter(|e| !SKIP.contains(&e.file_name().to_string_lossy().as_ref()))
                .map(|e| e.path())
                .collect();
            children.sort();
            children.reverse();
            stack.extend(children);
            continue;
        }
        let Ok(meta) = p.metadata() else { continue };
        if meta.len() > 2_000_000 {
            continue;
        }
        let Ok(bytes) = std::fs::read(&p) else {
            continue;
        };
        if bytes.iter().take(8000).any(|&b| b == 0) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        let shown = p.strip_prefix(&base).unwrap_or(&p).display().to_string();
        for (n, line) in text.lines().enumerate() {
            if line.contains(pattern) {
                hits.push(format!("{shown}:{}: {}", n + 1, line.trim()));
                if hits.len() >= GREP_CAP {
                    hits.push(format!("[stopped at {GREP_CAP} matches]"));
                    break;
                }
            }
        }
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
