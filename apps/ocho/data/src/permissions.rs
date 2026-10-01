//! Permission modes: how much a session may do before it asks. The
//! vocabulary is each provider's own and mirrors `internal/core/permissions.go`;
//! the CLI validates, so this list only drives pickers and labels.

use std::collections::HashMap;

use crate::launch::{provider_label, PROVIDERS};

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
        "antigravity" => &["accept-edits", "plan", "bypassPermissions"],
        "grok" => &[
            "acceptEdits",
            "auto",
            "dontAsk",
            "plan",
            "bypassPermissions",
        ],
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
        "acceptEdits" | "accept-edits" => "Accept edits".into(),
        "dontAsk" => "Don't ask".into(),
        "plan" => "Plan".into(),
        "bypassPermissions" => "Bypass permissions".into(),
        "ask" => "Ask".into(),
        "allow" => "Allow everything".into(),
        other => other.to_string(),
    }
}

/// The compact form for the composer's line: "YOLO", "Plan", "Read-only".
pub fn short(mode: &str) -> String {
    label(mode).split(" · ").next().unwrap_or("").to_string()
}

/// What the mode actually allows, in the words someone reading a menu needs.
pub fn describe(mode: &str) -> &'static str {
    match mode {
        "" => "whatever the machine is set to",
        NATIVE => "the provider's own behaviour",
        "read-only" => "reads and reasons, changes nothing",
        "auto" => "works inside the folder, asks to leave it",
        "full-access" => "edits and runs anywhere, no sandbox",
        "yolo" => "no approvals and no sandbox at all",
        "manual" => "asks before every edit and command",
        "acceptEdits" | "accept-edits" => "applies edits without asking, still asks to run things",
        "dontAsk" => "stops asking for the rest of the session",
        "plan" => "writes a plan first, touches nothing",
        "bypassPermissions" => "skips every permission check",
        "ask" => "asks before acting",
        "allow" => "allows everything",
        _ => "",
    }
}

/// Modes that let an agent act with no one watching. Worth colouring.
pub fn risky(mode: &str) -> bool {
    matches!(
        mode,
        "yolo" | "bypassPermissions" | "full-access" | "allow" | "dontAsk"
    )
}

/// The short form for a row: "Codex yolo · Claude auto", providers in a
/// fixed order, empty when nothing is set.
pub fn summary(defaults: &HashMap<String, String>) -> String {
    PROVIDERS
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
pub fn machine_spec(
    codex: &str,
    claude: &str,
    opencode: &str,
    antigravity: &str,
    grok: &str,
) -> String {
    [
        ("codex", codex),
        ("claude", claude),
        ("opencode", opencode),
        ("antigravity", antigravity),
        ("grok", grok),
    ]
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
            machine_spec("yolo", "", " auto ", "plan", "acceptEdits"),
            "codex=yolo,claude=default,opencode=auto,antigravity=plan,grok=acceptEdits"
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
        for provider in PROVIDERS {
            for mode in choices(provider) {
                assert_ne!(label(mode), *mode, "{provider} {mode}");
            }
        }
        assert_eq!(label("custom"), "custom");
    }

    #[test]
    fn the_line_says_modes_briefly_and_the_menu_says_what_they_allow() {
        assert_eq!(short("yolo"), "YOLO");
        assert_eq!(short("manual"), "Manual");
        assert_eq!(short("accept-edits"), "Accept edits");
        assert_eq!(short(""), "Machine default");
        for provider in PROVIDERS {
            for mode in choices(provider) {
                assert_ne!(describe(mode), "", "{provider} {mode}");
            }
        }
        assert_eq!(describe(""), "whatever the machine is set to");
        assert_eq!(describe("custom"), "");
        assert!(risky("dontAsk") && risky("allow") && risky("full-access"));
        assert!(!risky("plan") && !risky("") && !risky("auto"));
        let mut defaults = HashMap::new();
        defaults.insert("grok".to_string(), "plan".to_string());
        defaults.insert("antigravity".to_string(), "plan".to_string());
        assert_eq!(summary(&defaults), "Antigravity plan · Grok plan");
    }
}
