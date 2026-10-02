//! The app's native-module roster (LLP 1024 D1): the tags its module
//! artifact serves, declared in `app.json` as `"modules": ["tag", …]`. A
//! module tag the roster does not name is `bake-unknown-module`, so a typo
//! is a named diagnostic in the dev loop and in every build, never an empty
//! box at runtime. Beside it, the app's `data-*` words (LLP 1075.003 Q2):
//! `"data": ["word", …]`, from which the Apple build also writes the Swift
//! module's typed keys; a word the list lacks is `bake-undeclared-data`.

use super::{CompileError, Manifest};
use contract_syntax::{File, Span};
use std::path::Path;

/// The roster in `manifest`, each name checked against the tag grammar.
pub fn roster(manifest: &Manifest) -> Result<Vec<String>, String> {
    let Some(value) = manifest.json.get("modules") else {
        return Ok(Vec::new());
    };
    let names = value
        .as_array()
        .ok_or("app.json `modules` is a list of custom-element tags")?;
    names
        .iter()
        .map(|name| match name.as_str() {
            Some(tag) if contract_lower::is_module_tag(tag) => Ok(tag.to_owned()),
            _ => Err(format!(
                "app.json `modules`: {name} is not a lowercase custom-element name (LLP 1024 D1)"
            )),
        })
        .collect()
}

/// The `data-*` words in `manifest`, each checked against HTML's spelling
/// and the words a host already writes.
pub fn data_words(manifest: &Manifest) -> Result<Vec<String>, String> {
    let Some(value) = manifest.json.get("data") else {
        return Ok(Vec::new());
    };
    let words = value
        .as_array()
        .ok_or("app.json `data` is a list of data-* words, without the `data-`")?;
    words
        .iter()
        .map(|word| match word.as_str() {
            Some(w)
                if contract_lower::dataset::is_word(w)
                    && !contract_lower::dataset::reserved(w) =>
            {
                Ok(w.to_owned())
            }
            Some(w) if contract_lower::dataset::is_word(w) => Err(format!(
                "app.json `data`: `{w}` is written by the web host on its own elements (LLP 1075.003 §3.3)"
            )),
            _ => Err(format!(
                "app.json `data`: {word} is not a data-* word: lowercase words of letters and digits joined by `-`, without the `data-` (LLP 1075.003 Q2)"
            )),
        })
        .collect()
}

/// Every module tag and `data-` word in `file` against what the app at
/// `app_root` declares.
pub(super) fn check(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
    let mut errors = check_modules(file, app_root).err().unwrap_or_default();
    errors.extend(check_data(file, app_root).err().unwrap_or_default());
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_data(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
    let used = contract_lower::data_words(file);
    if used.is_empty() {
        return Ok(());
    }
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let declared = Manifest::read(app_root)
        .and_then(|m| data_words(&m))
        .map_err(|message| vec![refusal("app-manifest", message, Span::default())])?;
    let mut seen = std::collections::BTreeSet::new();
    let errors: Vec<CompileError> = used
        .into_iter()
        .filter(|(word, _)| {
            contract_lower::dataset::is_word(word)
                && !contract_lower::dataset::reserved(word)
                && !declared.contains(word)
                && seen.insert(word.clone())
        })
        .map(|(word, span)| {
            let hint = contract_syntax::suggestion(&word, declared.iter().map(String::as_str))
                .map(|n| format!("; did you mean `data-{n}`?"))
                .unwrap_or_default();
            let listed = if declared.is_empty() {
                "the app declares no data-* words (app.json `data`)".to_owned()
            } else {
                format!("the app's words are {}", declared.join(", "))
            };
            refusal(
                "bake-undeclared-data",
                format!("`data-{word}` is not declared: {listed}{hint}"),
                span,
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_modules(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
    let used = contract_lower::module_tags(file);
    if used.is_empty() {
        return Ok(());
    }
    let refusal = |id: &str, message: String, span: Span| CompileError {
        pass: "bake",
        id: id.into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    };
    let roster = Manifest::read(app_root)
        .and_then(|m| roster(&m))
        .map_err(|message| vec![refusal("app-manifest", message, Span::default())])?;
    let errors: Vec<CompileError> = used
        .into_iter()
        .filter(|(tag, _)| !roster.contains(tag))
        .map(|(tag, span)| {
            let hint = contract_syntax::suggestion(&tag, roster.iter().map(String::as_str))
                .map(|n| format!("; did you mean `{n}`?"))
                .unwrap_or_default();
            let listed = if roster.is_empty() {
                "the app declares no native modules (app.json `modules`)".to_owned()
            } else {
                format!("the app's modules are {}", roster.join(", "))
            };
            refusal(
                "bake-unknown-module",
                format!("`{tag}` is not a native module of this app: {listed}{hint}"),
                span,
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
