//! The app's native-module roster (LLP 1024 D1): the tags its module
//! artifact serves, declared in `app.json` as `"modules": ["tag", …]`. A
//! module tag the roster does not name is `bake-unknown-module`, so a typo
//! is a named diagnostic in the dev loop and in every build, never an empty
//! box at runtime.

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

/// Every module tag in `file` against the roster of the app at `app_root`.
pub(super) fn check(file: &File, app_root: &Path) -> Result<(), Vec<CompileError>> {
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
