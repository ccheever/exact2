//! A font directory's faces, cached: a phone's `/system/fonts` holds about
//! 300 files, and reading every one's name table on each launch was most of
//! a cold start's text setup (LLP 1076). The cache is a small text file in
//! the app's cache directory, keyed by the directory's listing (each file's
//! name, length and modification time), so a system update that changes a
//! font reads the directory again. Each face's `wght` range is kept with it:
//! font matching would otherwise open every file of another weight to learn
//! whether it is variable.
use cosmic_text::{fontdb, FontSystem};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const HEADER: &str = "exact-fonts 2";

/// A cached face and its `wght` range.
type Cached = (fontdb::FaceInfo, Option<(f32, f32)>);

/// Load `dir`'s faces into `fonts`, from the cache when the directory has
/// not changed since it was written.
pub(super) fn load(fonts: &mut FontSystem, dir: &str) {
    let Some(cache) = cache_file(dir) else {
        fonts.db_mut().load_fonts_dir(dir);
        return;
    };
    let key = listing(Path::new(dir));
    if let Some(faces) = std::fs::read_to_string(&cache)
        .ok()
        .and_then(|t| parse(&t, key))
    {
        let db = fonts.db_mut();
        let axes: Vec<_> = faces
            .into_iter()
            .map(|(face, axis)| (db.push_face_info(face), axis))
            .collect();
        fonts.set_weight_axes(axes);
        return;
    }
    let db = fonts.db_mut();
    let before: std::collections::HashSet<fontdb::ID> = db.faces().map(|f| f.id).collect();
    db.load_fonts_dir(dir);
    let mut out = format!("{HEADER} {key:016x}\n");
    let mut axes = Vec::new();
    for f in db.faces().filter(|f| !before.contains(&f.id)) {
        let fontdb::Source::File(path) = &f.source else {
            continue;
        };
        let families: Vec<&str> = f.families.iter().map(|(n, _)| n.as_str()).collect();
        if families.iter().any(|n| n.contains(['\t', '\n', '\u{1f}'])) {
            continue;
        }
        let axis = cosmic_text::face_weight_axis(db, f.id);
        axes.push((f.id, axis));
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            axis.map_or("-".to_string(), |(min, max)| format!("{min},{max}")),
            path.display(),
            f.index,
            f.post_script_name,
            match f.style {
                fontdb::Style::Normal => 0,
                fontdb::Style::Italic => 1,
                fontdb::Style::Oblique => 2,
            },
            f.weight.0,
            f.stretch.to_number(),
            u8::from(f.monospaced),
            families.join("\u{1f}"),
        ));
    }
    fonts.set_weight_axes(axes);
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Written whole, then moved into place: a reader never sees half of it.
    let tmp = cache.with_extension("tmp");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(&tmp, &cache);
    }
}

/// `$XDG_CACHE_HOME` or `$HOME/.cache`, `exact/fonts-<dir hash>`.
fn cache_file(dir: &str) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    dir.hash(&mut h);
    Some(
        base.join("exact")
            .join(format!("fonts-{:016x}", h.finish())),
    )
}

/// The directory's listing as a key: each entry's name, length and
/// modification time, in name order.
fn listing(dir: &Path) -> u64 {
    let mut entries: Vec<(String, u64, u128)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let t = m
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_nanos();
            Some((e.file_name().to_string_lossy().into_owned(), m.len(), t))
        })
        .collect();
    entries.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut h);
    h.finish()
}

/// The faces a cache file written for `key` lists, each with its `wght`
/// range, or `None` (stale, short, or not one).
fn parse(text: &str, key: u64) -> Option<Vec<Cached>> {
    let mut lines = text.lines();
    if lines.next()? != format!("{HEADER} {key:016x}") {
        return None;
    }
    lines
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let [axis, path, index, ps, style, weight, stretch, mono, families] = f[..] else {
                return None;
            };
            let axis = match axis {
                "-" => None,
                range => {
                    let (min, max) = range.split_once(',')?;
                    Some((min.parse().ok()?, max.parse().ok()?))
                }
            };
            let face = fontdb::FaceInfo {
                id: fontdb::ID::dummy(),
                source: fontdb::Source::File(PathBuf::from(path)),
                index: index.parse().ok()?,
                // Matching reads the names, not their language.
                families: families
                    .split('\u{1f}')
                    .map(|n| (n.to_string(), fontdb::Language::English_UnitedStates))
                    .collect(),
                post_script_name: ps.to_string(),
                style: match style {
                    "1" => fontdb::Style::Italic,
                    "2" => fontdb::Style::Oblique,
                    _ => fontdb::Style::Normal,
                },
                weight: fontdb::Weight(weight.parse().ok()?),
                stretch: stretch_of(stretch.parse().ok()?),
                monospaced: mono == "1",
            };
            Some((face, axis))
        })
        .collect()
}

fn stretch_of(n: u16) -> fontdb::Stretch {
    use fontdb::Stretch::*;
    match n {
        1 => UltraCondensed,
        2 => ExtraCondensed,
        3 => Condensed,
        4 => SemiCondensed,
        6 => SemiExpanded,
        7 => Expanded,
        8 => ExtraExpanded,
        9 => UltraExpanded,
        _ => Normal,
    }
}
