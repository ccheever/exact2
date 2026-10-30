//! `contract tamber export dtcg <app dir | app.contract | file.tamber> [-o
//! <file>]` (LLP 1121 D14, stage 5): the design as DTCG Format 2025.10 with
//! the Resolver Module (`exact_tamber::export`), printed or written to `-o`.
//! Each export keeps what it wrote as the baseline a re-import compares with,
//! by the design's digest, in `.exact/tamber-dtcg/` beside the design, whose
//! own `.gitignore` keeps it out of every commit.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str =
    "usage: contract tamber export dtcg <app dir | app.contract | file.tamber> [-o <file>]";

pub fn run(args: &[String]) -> ExitCode {
    let (mut kind, mut input, mut out) = (None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" if i + 1 < args.len() => {
                out = Some(args[i + 1].clone());
                i += 2;
            }
            word if !word.starts_with('-') && kind.is_none() => {
                kind = Some(word.to_string());
                i += 1;
            }
            word if !word.starts_with('-') && input.is_none() => {
                input = Some(word.to_string());
                i += 1;
            }
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(kind), Some(input)) = (kind, input) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    if kind != "dtcg" {
        eprintln!("contract tamber export: no exporter `{kind}` (dtcg)");
        return ExitCode::from(2);
    }
    let Some(path) = crate::tamber::design(Path::new(&input)) else {
        eprintln!("contract tamber export: {input} has no design (an app.tamber, or the file app.json's \"tamber\" names)");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("contract tamber export: {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let exported = match export(&text) {
        Ok(e) => e,
        Err(all) => {
            for d in &all {
                eprintln!("{}:{d}", path.display());
            }
            return ExitCode::from(1);
        }
    };
    let kept = match keep(&path, &exported) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("contract tamber export: the baseline: {e}");
            return ExitCode::from(2);
        }
    };
    match out {
        Some(out) => match std::fs::write(&out, &exported) {
            Ok(()) => {
                eprintln!("{out}: written; its baseline {}", kept.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("contract tamber export: {out}: {e}");
                ExitCode::from(2)
            }
        },
        None => {
            print!("{exported}");
            ExitCode::SUCCESS
        }
    }
}

/// Design `text` as a DTCG export.
pub fn export(text: &str) -> Result<String, Vec<exact_tamber::Diagnostic>> {
    let vocab = contract_lower::tamber_vocab();
    let folding = exact_tamber::check(text, &vocab)?;
    exact_tamber::export::dtcg(&folding, &vocab)
}

/// How many exports' baselines are kept, the newest.
const KEPT: usize = 16;

/// Where the baselines of the design at `design` are kept: `.exact/` beside
/// it, a directory of its own.
pub fn baselines(design: &Path) -> PathBuf {
    design
        .parent()
        .unwrap_or(Path::new("."))
        .join(".exact")
        .join(exact_tamber::export::BASELINES)
}

/// A directory, refused when it is a symbolic link: an export never writes
/// through one.
fn plain_dir(dir: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_symlink() || !m.is_dir() => Err(std::io::Error::other(format!(
            "{} is not a plain directory",
            dir.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => std::fs::create_dir(dir),
        Err(e) => Err(e),
    }
}

/// Keep `exported` as the baseline of its design's digest, in a directory
/// whose own `.gitignore` (`*`) keeps it out of every commit, written by a
/// rename so a link in its place is replaced, never followed; the oldest
/// past [`KEPT`] go. Its path.
pub fn keep(design: &Path, exported: &str) -> std::io::Result<PathBuf> {
    let dir = baselines(design);
    plain_dir(dir.parent().expect("in .exact"))?;
    plain_dir(&dir)?;
    let digest = exact_tamber::export::digest(exported)
        .filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| std::io::Error::other("the export names no digest"))?;
    // A new file of its own (`create_new` follows no link), renamed into
    // place: a link at either name is replaced, never written through.
    let write = |name: &str, text: &str| {
        use std::io::Write;
        let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
        let _ = std::fs::remove_file(&tmp);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        drop(f);
        std::fs::rename(&tmp, dir.join(name))
    };
    write(".gitignore", "*\n")?;
    let file = format!("{digest}.json");
    write(&file, exported)?;
    let mut all = kept_files(&dir);
    for (path, _) in all.drain(KEPT.min(all.len())..) {
        let _ = std::fs::remove_file(path);
    }
    Ok(dir.join(file))
}

/// All the kept baselines read, at most (an import reads them in a
/// process of its own, but a full directory still costs it nothing past this).
const KEPT_BYTES: u64 = 64 << 20;

/// The baselines kept for the design at `design`, the newest first: the
/// newest [`KEPT`], and only while they total [`KEPT_BYTES`]; none when
/// `.exact` or its baselines directory is a link (never read through one,
/// as [`keep`] never writes through one).
pub fn kept(design: &Path) -> Vec<String> {
    kept_within(design, KEPT_BYTES)
}

fn kept_within(design: &Path, mut left: u64) -> Vec<String> {
    use std::io::Read;
    let dir = baselines(design);
    let plain = |d: &Path| std::fs::symlink_metadata(d).is_ok_and(|m| m.is_dir());
    if !plain(dir.parent().expect("in .exact")) || !plain(&dir) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (path, _) in kept_files(&dir).into_iter().take(KEPT) {
        let Ok(f) = std::fs::File::open(path) else {
            continue;
        };
        // Every byte read is charged, a file that is not text too; one
        // byte past what is left ends the reading.
        let mut bytes = Vec::new();
        let read = f.take(left + 1).read_to_end(&mut bytes);
        let n = bytes.len() as u64;
        if n > left {
            break;
        }
        left -= n;
        // (a read that failed part way is charged, never kept)
        if let (Ok(_), Ok(text)) = (read, String::from_utf8(bytes)) {
            out.push(text);
        }
    }
    out
}

fn kept_files(dir: &Path) -> Vec<(PathBuf, std::time::SystemTime)> {
    let mut all: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    all.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
    all
}

#[cfg(test)]
mod kept_tests {
    use super::baselines;
    #[cfg(unix)]
    use super::{kept, KEPT};

    /// The import's read of the kept baselines: the newest [`KEPT`], none
    /// through a linked directory (stage 10b E2+E3 round 9's leftover).
    #[cfg(unix)]
    #[test]
    fn kept_reads_the_newest_and_never_through_a_link() {
        let root = std::env::temp_dir().join(format!("exact-kept-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (app, elsewhere) = (root.join("app"), root.join("elsewhere"));
        std::fs::create_dir_all(&app).unwrap();
        let design = app.join("app.tamber");
        let dir = baselines(&design);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..KEPT + 4 {
            std::fs::write(dir.join(format!("{i:02}.json")), format!("{{\"n\": {i}}}")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5)); // (each newer)
        }
        let read = kept(&design);
        assert_eq!(read.len(), KEPT);
        assert_eq!(
            read[0],
            format!("{{\"n\": {}}}", KEPT + 3),
            "the newest first"
        );
        // `.exact` a link to another directory with baselines: none read.
        std::fs::rename(app.join(".exact"), &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, app.join(".exact")).unwrap();
        assert!(kept(&design).is_empty());
        // `.exact` real, its baselines directory a link: none read either.
        std::fs::remove_file(app.join(".exact")).unwrap();
        std::fs::create_dir_all(app.join(".exact")).unwrap();
        std::os::unix::fs::symlink(elsewhere.join(dir.file_name().unwrap()), &dir).unwrap();
        assert!(kept(&design).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Read up to a total only, a file that is not text charged too.
    #[test]
    fn kept_stops_at_its_total() {
        let root = std::env::temp_dir().join(format!("exact-kept-cap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let design = root.join("app.tamber");
        let dir = baselines(&design);
        std::fs::create_dir_all(&dir).unwrap();
        // Oldest to newest: a text file, then one not text, then a newer text file.
        std::fs::write(dir.join("a.json"), "{\"n\": 1}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(dir.join("b.json"), [0xff; 100]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(dir.join("c.json"), "{\"n\": 3}").unwrap();
        // 8 bytes, then 100 not read as text but charged, then 8 more: 116 in all.
        assert_eq!(
            super::kept_within(&design, 8 + 100 + 8),
            ["{\"n\": 3}", "{\"n\": 1}"]
        );
        assert_eq!(super::kept_within(&design, 8 + 100 + 7), ["{\"n\": 3}"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
