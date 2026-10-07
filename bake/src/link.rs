//! What an Apple archive links (LLP 1047.001 D2, D3; LLP 1047 D8).
//!
//! @ref LLP 1047 D8 (on native the linked set is a compatibility input; the
//! manifest may link capabilities ahead of any plan that uses them)
//! @ref LLP 1047.001 D2 (one composition drives Rust, Swift, admission and
//! compatibility)

use crate::compat::{compatibility_digest, Compat};
use exact_runner::{Capability, Uses};

/// The capabilities an Apple archive links, named into `compat`'s inputs
/// (`linked`, which moves its id), and the entry's text for them
/// ([`contract::apple_linked`]).
///
/// A production build (`host/apple/build.mjs` says `EXACT_APPLE_LINK=plan`)
/// links what the plan uses and what the manifest's `link` names ahead of
/// use; every other build, and a build `build.mjs` does not drive, links
/// every capability (LLP 1047.001 D6). A bundle whose plan uses a capability
/// this binary lacks is then another compatibility id: a binary change.
pub fn apple_link(
    compat: &mut Compat,
    plan: &exact_plan::Plan,
    manifest: &contract::Manifest,
    host: &str,
) -> Result<String, String> {
    println!("cargo:rerun-if-env-changed=EXACT_APPLE_LINK");
    let uses = if std::env::var("EXACT_APPLE_LINK").is_ok_and(|v| v == "plan") {
        ahead(manifest)?
            .into_iter()
            .fold(exact_runner::uses(plan), Uses::with)
    } else {
        Capability::ALL.into_iter().fold(Uses::NONE, Uses::with)
    };
    let names: Vec<&str> = uses.iter().map(Capability::name).collect();
    compat.inputs["linked"] = serde_json::json!(names);
    compat.id = compatibility_digest(&compat.inputs);
    Ok(contract::apple_linked(uses, host))
}

/// The manifest's `link`: capabilities by name, linked before any plan uses
/// them, so a later bundle may (LLP 1047 D8).
pub fn ahead(manifest: &contract::Manifest) -> Result<Vec<Capability>, String> {
    let Some(link) = manifest.json.get("link") else {
        return Ok(Vec::new());
    };
    let names = link
        .as_array()
        .ok_or("app.json: `link` is a list of capability names")?;
    names
        .iter()
        .map(|name| {
            name.as_str()
                .and_then(Capability::from_name)
                .ok_or_else(|| {
                    let known: Vec<&str> = Capability::ALL.iter().map(|c| c.name()).collect();
                    format!("app.json: `link` names {name}, which is not a capability; capabilities: {}", known.join(", "))
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ahead;
    use exact_runner::Capability;

    fn manifest(json: serde_json::Value) -> contract::Manifest {
        contract::Manifest {
            json,
            id: "com.example.link".into(),
            name: "Link".into(),
            declared: true,
        }
    }

    #[test]
    fn link_names_capabilities_and_refuses_another_word() {
        assert!(ahead(&manifest(serde_json::json!({}))).unwrap().is_empty());
        assert_eq!(
            ahead(&manifest(
                serde_json::json!({"link": ["markdown", "grouped_lists"]})
            ))
            .unwrap(),
            vec![Capability::Markdown, Capability::GroupedLists]
        );
        let refused = ahead(&manifest(serde_json::json!({"link": ["markdwon"]}))).unwrap_err();
        assert!(
            refused.contains("markdwon") && refused.contains("markdown"),
            "{refused}"
        );
    }

    /// The manifest schema's `link` names are the roster's, in its order.
    #[test]
    fn the_schema_names_every_capability() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../scripts/app.schema.json")).unwrap();
        let names: Vec<&str> = schema["properties"]["link"]["items"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_str().unwrap())
            .collect();
        let roster: Vec<&str> = Capability::ALL.iter().map(|c| c.name()).collect();
        assert_eq!(names, roster);
    }
}
