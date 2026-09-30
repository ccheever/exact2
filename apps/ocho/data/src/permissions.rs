//! Permission modes: how much a session may do before it asks. The
//! vocabulary is each provider's own and mirrors `internal/core/permissions.go`;
//! the CLI validates, so this list only drives pickers and labels.

use std::collections::HashMap;

use crate::launch::provider_label;

/// The explicit request for a provider's native behavior, as the CLI spells it.
pub const NATIVE: &str = "default";

/// Modes a provider accepts, in the order the pickers show them.
pub fn choices(provider: &str) -> &'static [&'static str] {
    match provider {
        "codex" => &["read-only", "auto", "full-access", "yolo"],
        "claude" => &[
            "manual",
            "acceptEdits",
            "auto",
            "dontAsk",
            "plan",
            "bypassPermissions",
        ],
        "opencode" => &["ask", "allow"],
        _ => &[],
    }
}

/// How a mode reads in a chip, row or list.
pub fn label(mode: &str) -> String {
    match mode {
        "" => "Machine default".into(),
        NATIVE => "Provider default".into(),
        "read-only" => "Read-only".into(),
        "auto" => "Auto".into(),
        "full-access" => "Full access".into(),
        "yolo" => "YOLO · no approvals, no sandbox".into(),
        "manual" => "Manual · always ask".into(),
        "acceptEdits" => "Accept edits".into(),
        "dontAsk" => "Don't ask".into(),
        "plan" => "Plan".into(),
        "bypassPermissions" => "Bypass permissions".into(),
        "ask" => "Ask".into(),
        "allow" => "Allow everything".into(),
        other => other.to_string(),
    }
}

/// The short form for a row: "Codex yolo · Claude auto", providers in a
/// fixed order, empty when nothing is set.
pub fn summary(defaults: &HashMap<String, String>) -> String {
    ["codex", "claude", "opencode"]
        .into_iter()
        .filter_map(|provider| {
            defaults
                .get(provider)
                .filter(|mode| !mode.is_empty())
                .map(|mode| format!("{} {mode}", provider_label(provider)))
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The `--permissions` assignment list for `fleet machine edit`: every
/// provider is named, so a cleared field removes that provider's default.
pub fn machine_spec(codex: &str, claude: &str, opencode: &str) -> String {
    [("codex", codex), ("claude", claude), ("opencode", opencode)]
        .into_iter()
        .map(|(provider, mode)| {
            let mode = if mode.trim().is_empty() {
                NATIVE
            } else {
                mode.trim()
            };
            format!("{provider}={mode}")
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_spec_names_every_provider_so_blanks_clear() {
        assert_eq!(
            machine_spec("yolo", "", " auto "),
            "codex=yolo,claude=default,opencode=auto"
        );
    }

    #[test]
    fn summary_keeps_provider_order_and_skips_unset() {
        let mut defaults = HashMap::new();
        assert_eq!(summary(&defaults), "");
        defaults.insert("claude".to_string(), "auto".to_string());
        defaults.insert("codex".to_string(), "yolo".to_string());
        defaults.insert("opencode".to_string(), String::new());
        assert_eq!(summary(&defaults), "Codex yolo · Claude auto");
    }

    #[test]
    fn every_mode_has_a_readable_label() {
        for provider in ["codex", "claude", "opencode"] {
            for mode in choices(provider) {
                assert_ne!(label(mode), *mode, "{provider} {mode}");
            }
        }
        assert_eq!(label("custom"), "custom");
    }
}
