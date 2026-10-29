//! Property fixtures keep assertions in the test file beside their program.

use crate::script::{Case, Scripted};
use std::path::Path;

pub(crate) fn cases(
    file: &Path,
    name: &str,
    source: &str,
) -> Result<Option<Vec<Scripted>>, String> {
    let test_file = file.with_extension("test.contract");
    if !test_file.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&test_file).map_err(|e| e.to_string())?;
    let properties = contract::properties(&text).map_err(|e| e.to_string())?;
    let checked = contract::check_properties_path(file, &properties);
    if let Some(reason) = directive(source, "refused").first() {
        let errors = checked.err().ok_or_else(|| {
            format!(
                "{}: expected a property read refusal containing {reason:?}",
                file.display()
            )
        })?;
        if !errors.iter().any(|e| e.message.contains(reason)) {
            return Err(format!(
                "{}: expected {reason:?}; got {errors:?}",
                file.display()
            ));
        }
        println!(
            "PROPERTY REFUSED {}: {} (expected)",
            test_file.display(),
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
        return Ok(Some(Vec::new()));
    }
    checked.map_err(|errors| {
        format!(
            "{}: {}",
            test_file.display(),
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let tests = contract::tests(&text).map_err(|e| e.to_string())?;
    if tests.is_empty() {
        return Ok(Some(vec![crate::explore(name, source, Some(file))]));
    }
    tests
        .iter()
        .map(|t| crate::script::test_case(name, source, Some(file), t, None))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

pub(crate) fn check(case: &Case, observation: &[String], announce: bool) -> Result<(), String> {
    let Some(path) = &case.path else {
        return Ok(());
    };
    // These directives are corpus fixtures, never exemptions for an app's verify.
    if !path.components().any(|c| c.as_os_str() == "corpus") {
        return Ok(());
    }
    let test_file = path.with_extension("test.contract");
    if !test_file.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(test_file).map_err(|e| e.to_string())?;
    let properties = contract::properties(&text).map_err(|e| e.to_string())?;
    let report = crate::property::evaluate(&properties, observation);
    let expected = directive(&case.source, "false");
    let false_names: std::collections::BTreeSet<_> = report
        .failures
        .iter()
        .map(|f| properties[f.property].name.as_str())
        .collect();
    for name in &expected {
        if !false_names.contains(name.as_str()) {
            return Err(format!(
                "property {name:?} was expected false but was not falsified"
            ));
        }
    }
    if announce {
        for name in &expected {
            let f = report
                .failures
                .iter()
                .find(|f| properties[f.property].name == *name)
                .expect("expected failure was found");
            println!(
                "PROPERTY {name:?} is false at step {}: {} (expected; {})",
                f.step, f.observed, case.name
            );
        }
    }
    for f in &report.failures {
        let p = &properties[f.property];
        if !expected.contains(&p.name) {
            return Err(format!(
                "PROPERTY {:?} is false at step {}: {}",
                p.name, f.step, f.observed
            ));
        }
    }
    Ok(())
}

fn directive(source: &str, kind: &str) -> Vec<String> {
    let prefix = format!("// # expect property {kind} ");
    source
        .lines()
        .filter_map(|l| {
            l.strip_prefix(&prefix)?
                .strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_string)
        })
        .collect()
}
