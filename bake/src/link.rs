//! What an Apple archive links (LLP 1047.001 D2, D3; LLP 1047 D8).
//!
//! @ref LLP 1047 D8 (on native the linked set is a compatibility input; the
//! manifest may link capabilities ahead of any plan that uses them)
//! @ref LLP 1047.001 D2 (one composition drives Rust, Swift, admission and
//! compatibility)

use crate::compat::{compatibility_digest, Compat};
use exact_runner::{Capability, Uses};

/// Name what an Apple archive links into `compat`'s inputs (`linked`), from
/// the plan its build script baked into `out`, before the id is taken and
/// everything that binds it (the GPU module's card, the receipts) is written.
///
/// A production build (`host/apple/build.mjs` says `EXACT_APPLE_LINK=plan`)
/// links what the plan uses and what the manifest's `link` names ahead of
/// use; every other build, and a build `build.mjs` does not drive, links
/// every capability (LLP 1047.001 D6). A bundle whose plan uses a capability
/// this binary lacks is then another compatibility id: a binary change.
pub(crate) fn name_into(
    compat: &mut Compat,
    manifest: &contract::Manifest,
    out: &std::path::Path,
) -> Result<(), String> {
    println!("cargo:rerun-if-env-changed=EXACT_APPLE_LINK");
    let uses = if std::env::var("EXACT_APPLE_LINK").is_ok_and(|v| v == "plan") {
        let bytes = std::fs::read(out.join("app.plan")).map_err(|e| {
            format!("the app bake must write OUT_DIR/app.plan before compatibility_id: {e}")
        })?;
        let plan = exact_plan::Plan::decode(&bytes)
            .map_err(|e| format!("the baked plan is invalid: {e:?}"))?;
        // The app's I/O grants link the host's I/O whatever the plan says:
        // a data source's requests are the source's, not the plan's.
        let io = compat.inputs["grantCeiling"]
            .as_str()
            .is_some_and(|ceiling| !exact_runner::io_grants(ceiling).is_empty());
        let uses = ahead(manifest)?
            .into_iter()
            .fold(exact_runner::uses(&plan), Uses::with);
        if io {
            uses.with(Capability::Io)
        } else {
            uses
        }
    } else {
        Capability::ALL.into_iter().fold(Uses::NONE, Uses::with)
    };
    let names: Vec<&str> = uses.iter().map(Capability::name).collect();
    compat.inputs["linked"] = serde_json::json!(names);
    compat.id = compatibility_digest(&compat.inputs);
    Ok(())
}

/// The Apple entry's text for what `compat` says the archive links
/// ([`contract::apple_linked`]); every capability where it says nothing.
pub fn apple_link(compat: &Compat, host: &str) -> String {
    let uses = match compat.inputs["linked"].as_array() {
        Some(names) => names
            .iter()
            .filter_map(|n| n.as_str().and_then(Capability::from_name))
            .fold(Uses::NONE, Uses::with),
        None => Capability::ALL.into_iter().fold(Uses::NONE, Uses::with),
    };
    contract::apple_linked(uses, host)
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
