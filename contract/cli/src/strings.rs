//! The app's strings tables (LLP 1060 D1): `strings/<locale>.json` beside
//! `app.contract`, each a flat `{ "key": "text with {name}" }`, the base
//! named by `app.json`'s `strings.base` (`en` when it names none). Every
//! table is checked on every compile, whether or not a `t` reads it: the
//! app's TypeScript imports the same files.

use super::{CompileError, Manifest};
use contract_syntax::Span;
use contract_types::strings::Strings;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

/// The tables, `None` for an app without a `strings` directory.
/// `source` is the root `app.contract`, which a manifest refusal names as
/// the driver's own does.
pub(super) fn load(
    app_root: &Path,
    source: &Path,
) -> Result<Option<Arc<Strings>>, Vec<CompileError>> {
    let refusal = |id: &str, file: &Path, message: String| CompileError {
        pass: "strings",
        id: id.into(),
        message,
        span: Span::default(),
        file: Some(file.to_path_buf()),
        related: Box::new([]),
    };
    let manifest = app_root.join("app.json");
    let declared = if manifest.is_file() {
        let json = Manifest::read(app_root)
            .map_err(|message| {
                vec![CompileError {
                    pass: "app",
                    ..refusal("app-manifest", source, message)
                }]
            })?
            .json;
        match json.pointer("/strings/base") {
            None => None,
            Some(serde_json::Value::String(base)) if is_locale(base) => Some(base.clone()),
            Some(_) => {
                return Err(vec![refusal(
                    "strings-base",
                    &manifest,
                    "`strings.base` is a BCP 47 locale, as in \"en\" or \"pt-BR\"".into(),
                )])
            }
        }
    } else {
        None
    };
    let dir = app_root.join("strings");
    let base = declared.clone().unwrap_or_else(|| "en".into());
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && declared.is_none() => {
            return Ok(None)
        }
        Err(e) => return Err(vec![refusal("strings-unreadable", &dir, e.to_string())]),
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    let mut errors = Vec::new();
    let mut tables = BTreeMap::new();
    for path in &paths {
        let locale = path.file_stem().unwrap_or_default().to_string_lossy();
        if !is_locale(&locale) {
            errors.push(refusal(
                "strings-locale",
                path,
                format!("`{locale}` is not a BCP 47 locale; name a table as in `en.json` or `pt-BR.json`"),
            ));
            continue;
        }
        if tables
            .keys()
            .any(|have: &String| have.eq_ignore_ascii_case(&locale))
        {
            errors.push(refusal(
                "strings-locale",
                path,
                format!("a second table for `{locale}`"),
            ));
            continue;
        }
        match read_table(path) {
            Ok(table) => {
                tables.insert(locale.into_owned(), table);
            }
            Err(message) => errors.push(refusal("strings-table", path, message)),
        }
    }
    let Some(base_table) = tables.get(&base) else {
        errors.push(refusal(
            "strings-base-missing",
            &dir.join(format!("{base}.json")),
            format!("the base locale is `{base}`, and it has no table"),
        ));
        return Err(errors);
    };
    // A translation may lag the base, never lead it: a key or placeholder
    // the base lacks is a typo or a leftover no `t` could ever show.
    for (locale, table) in tables.iter().filter(|(l, _)| **l != base) {
        let path = dir.join(format!("{locale}.json"));
        for (key, text) in table {
            let Some(base_text) = base_table.get(key) else {
                errors.push(refusal(
                    "strings-unknown-key",
                    &path,
                    format!("\"{key}\" is not in {base}.json"),
                ));
                continue;
            };
            let known: Vec<&str> = exact_plan::strings::placeholders(base_text).collect();
            for name in exact_plan::strings::placeholders(text).filter(|n| !known.contains(n)) {
                errors.push(refusal(
                    "strings-placeholder",
                    &path,
                    format!("\"{key}\" has `{{{name}}}`, which its {base}.json text does not"),
                ));
            }
        }
    }
    if !errors.is_empty() {
        errors.truncate(super::MAX_DIAGNOSTICS);
        return Err(errors);
    }
    Ok(Some(Arc::new(Strings { base, tables })))
}

fn read_table(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let flat = || "a table is one flat object of texts: { \"key\": \"text\" }".to_string();
    json.as_object()
        .ok_or_else(flat)?
        .iter()
        .map(|(key, text)| Ok((key.clone(), text.as_str().ok_or_else(flat)?.to_owned())))
        .collect()
}

/// `language(-subtag)*`, the shape a host reports (LLP 1027.000.000).
fn is_locale(tag: &str) -> bool {
    let mut subtags = tag.split('-');
    let language = subtags.next().unwrap_or_default();
    (2..=8).contains(&language.len())
        && language.bytes().all(|b| b.is_ascii_alphabetic())
        && subtags
            .all(|s| (1..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric()))
}
