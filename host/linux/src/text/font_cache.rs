//! A font directory's faces, cached: a phone's `/system/fonts` holds about
//! 300 files, and reading every one's tables on each launch was most of a
//! cold start's text setup (LLP 1076). The cache is a small text file in the
//! app's cache directory, keyed by the directory's listing (each file's
//! path, length and modification time, subdirectories included), so a
//! system update that changes a font reads the directory again.
//!
//! Each face is kept as fontique matches it (LLP 1085.000 §4, "The font
//! cache"): its family, width, style and weight, every variation axis, and
//! where its charmap is. A cached launch registers the faces described and
//! opens no font file; one is mapped when a shape first uses it.
use super::fonts::Described;
use fontique::{AxisInfo, CharmapIndex, FontStyle, FontWeight, FontWidth, SourceKind};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const HEADER: &str = "exact-fontique 1";

/// `dir`'s faces, from the cache when the directory has not changed since
/// it was written; otherwise scanned (every file read once) and cached.
pub(super) fn load(dir: &Path) -> Vec<Described> {
    if !dir.is_dir() {
        return Vec::new();
    }
    let key = listing(dir);
    let cache = cache_file(dir);
    if let Some(faces) = cache
        .as_ref()
        .and_then(|c| std::fs::read_to_string(c).ok())
        .and_then(|t| parse(&t, key))
    {
        return faces;
    }
    let faces = scan(dir);
    if let Some(cache) = cache {
        write(&cache, key, &faces);
    }
    faces
}

/// Every face under `dir`, read by fontique (the patched scan registers each
/// file once; a face fontique refuses, LLP 1085.000 G2, is not listed).
pub(super) fn scan(dir: &Path) -> Vec<Described> {
    let mut collection = super::fonts::empty_collection();
    collection.load_fonts_from_paths([dir]);
    let mut names: Vec<String> = collection.family_names().map(str::to_owned).collect();
    names.sort();
    let mut faces = Vec::new();
    for name in names {
        let Some(family) = collection.family_by_name(&name) else {
            continue;
        };
        for font in family.fonts() {
            let SourceKind::Path(path) = font.source().kind() else {
                continue;
            };
            faces.push(Described {
                family: name.clone(),
                path: path.clone(),
                index: font.index(),
                width: font.width(),
                style: font.style(),
                weight: font.weight(),
                axes: font.axes().to_vec(),
                charmap: font.charmap_index(),
            });
        }
    }
    faces
}

fn write(cache: &Path, key: u64, faces: &[Described]) {
    let mut out = format!("{HEADER} {key:016x}\n");
    for f in faces {
        if f.family.contains(['\t', '\n']) || f.path.to_string_lossy().contains(['\t', '\n']) {
            continue;
        }
        let (offset, symbol, mac) = f.charmap.to_parts();
        let axes: Vec<String> = f
            .axes
            .iter()
            .map(|a| format!("{}:{}:{}:{}", a.tag, a.min, a.max, a.default))
            .collect();
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{},{},{}\t{}\n",
            f.family,
            f.path.display(),
            f.index,
            f.width.ratio(),
            style_str(f.style),
            f.weight.value(),
            offset,
            u8::from(symbol),
            u8::from(mac),
            axes.join(";"),
        ));
    }
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Written whole under a name of this writer's own, then moved into
    // place: a reader never sees half of it, and two writers never share a
    // temporary file.
    static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = cache.with_extension(format!("tmp-{}-{serial}", std::process::id()));
    if std::fs::write(&tmp, out).is_ok() && std::fs::rename(&tmp, cache).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn style_str(s: FontStyle) -> String {
    match s {
        FontStyle::Normal => "n".into(),
        FontStyle::Italic => "i".into(),
        FontStyle::Oblique(None) => "o".into(),
        FontStyle::Oblique(Some(a)) => format!("o{a}"),
    }
}

fn style_of(s: &str) -> Option<FontStyle> {
    Some(match s {
        "n" => FontStyle::Normal,
        "i" => FontStyle::Italic,
        "o" => FontStyle::Oblique(None),
        o => FontStyle::Oblique(Some(o.strip_prefix('o')?.parse().ok()?)),
    })
}

/// `$XDG_CACHE_HOME` or `$HOME/.cache`, `exact/fonts-<dir hash>`.
fn cache_file(dir: &Path) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    HEADER.hash(&mut h);
    dir.hash(&mut h);
    Some(
        base.join("exact")
            .join(format!("fonts-{:016x}", h.finish())),
    )
}

/// The directory's listing as a key: each font file's path, length and
/// modification time, in path order, subdirectories included (as the scan
/// reads them).
fn listing(dir: &Path) -> u64 {
    fn walk(dir: &Path, depth: u32, out: &mut Vec<(String, u64, u128)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(m) = std::fs::metadata(&path) else {
                continue;
            };
            if m.is_dir() {
                if depth < 16 {
                    walk(&path, depth + 1, out);
                }
                continue;
            }
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            out.push((path.to_string_lossy().into_owned(), m.len(), t));
        }
    }
    let mut entries = Vec::new();
    walk(dir, 0, &mut entries);
    entries.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut h);
    h.finish()
}

/// The faces a cache file written for `key` lists, or `None` (stale,
/// short, or not one).
fn parse(text: &str, key: u64) -> Option<Vec<Described>> {
    let mut lines = text.lines();
    if lines.next()? != format!("{HEADER} {key:016x}") {
        return None;
    }
    let mut paths: std::collections::HashMap<&str, Arc<Path>> = Default::default();
    lines
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [family, path, index, width, style, weight, cmap, axes] = f[..] else {
                return None;
            };
            let path = paths
                .entry(path)
                .or_insert_with(|| Arc::from(Path::new(path)))
                .clone();
            let mut cm = cmap.split(',');
            let charmap = CharmapIndex::from_parts(
                cm.next()?.parse().ok()?,
                cm.next()? == "1",
                cm.next()? == "1",
            );
            let axes = axes
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|a| {
                    let p: Vec<&str> = a.split(':').collect();
                    let [tag, min, max, default] = p[..] else {
                        return None;
                    };
                    Some(AxisInfo {
                        tag: tag.parse().ok()?,
                        min: min.parse().ok()?,
                        max: max.parse().ok()?,
                        default: default.parse().ok()?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Described {
                family: family.to_string(),
                path,
                index: index.parse().ok()?,
                width: FontWidth::from_ratio(width.parse().ok()?),
                style: style_of(style)?,
                weight: FontWeight::new(weight.parse().ok()?),
                axes,
                charmap,
            })
        })
        .collect()
}
