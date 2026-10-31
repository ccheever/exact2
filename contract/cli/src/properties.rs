//! Authored tests and property read validation.

use crate::CompileError;
use contract_syntax::{PropertyDecl, Step, TestDecl};
use std::path::Path;

/// Parse properties without compiling an application.
pub fn properties(src: &str) -> Result<Vec<PropertyDecl>, CompileError> {
    Ok(contract_syntax::parse(src)?.properties)
}

/// Check property reads against the app's expanded root before running scripts.
pub fn check_properties_path(
    path: &Path,
    properties: &[PropertyDecl],
) -> Result<(), Vec<CompileError>> {
    let (file, expanded) = property_program(path)?;
    let errors: Vec<_> = contract_analyze::check_properties(&expanded.root, &file.fns, properties)
        .into_iter()
        .map(CompileError::from)
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check observed suggestions against every write and caller before printing them.
pub fn check_suggestions_path(
    path: &Path,
    properties: &[PropertyDecl],
) -> Result<Vec<contract_analyze::SuggestionCheck>, Vec<CompileError>> {
    let (file, expanded) = property_program(path)?;
    Ok(contract_analyze::check_suggestions(
        &expanded, &file, properties,
    ))
}

fn property_program(
    path: &Path,
) -> Result<(contract_syntax::File, contract_syntax::Expanded), Vec<CompileError>> {
    // A relative path makes the `use` containment check compare a canonical
    // sibling against a relative root and refuse it as leaving the app directory
    // (`docs/agent-pitfalls.md`); the device runner passes the test file as typed.
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = canonical.as_path();
    let src = crate::read_source(path)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let (file, _) = crate::sources::load(path, &src, root)?;
    let expanded = contract_syntax::expand(&file).map_err(CompileError::from)?;
    Ok((file, expanded))
}

/// The `test` blocks of a file (LLP 1017 P7) — normally `app.test.contract`
/// beside the app, holding nothing else. Parsed, never compiled: a test is a
/// script for the agent driver (`scripts/agent.mjs --test`), and its steps
/// are the eight operations plus `expect` lines that read their replies.
/// The file's top-level launch lines lead each test's steps, unless the test
/// names the same fact itself (habits F7, calendar F13).
pub fn tests(src: &str) -> Result<Vec<TestDecl>, CompileError> {
    let file = contract_syntax::parse(src)?;
    Ok(file
        .tests
        .into_iter()
        .map(|mut test| {
            // By fact, among the test's own leading launch lines: a `fail
            // fetch` line by its prefix (LLP 1103 D3); a later one is a step.
            let leading = test
                .steps
                .iter()
                .take_while(|s| contract_syntax::is_launch(s));
            let own = |l: &Step| leading.clone().any(|s| contract_syntax::same_launch(s, l));
            let inherited = file
                .launch
                .iter()
                .filter(|l| !own(l))
                .cloned()
                .collect::<Vec<_>>();
            test.steps = inherited.into_iter().chain(test.steps).collect();
            test
        })
        .collect())
}
