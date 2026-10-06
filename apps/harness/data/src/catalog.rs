//! OpenRouter's model catalog (`GET /api/v1/models`, no key needed) as the
//! picker's choices: models that take tools, from the providers a coding
//! agent is worth trying, the newest two of each, without the `:free`,
//! preview, audio, image and search variants when a plain one exists.
//! The JSON is cached in the configuration directory; a static list stands
//! in when neither the network nor the cache has one.

use crate::state::ModelChoice;
use serde_json::Value as Json;
use std::path::Path;
use std::time::Duration;

/// The upstream providers kept, in the picker's order.
pub const PROVIDERS: [&str; 10] = [
    "anthropic",
    "openai",
    "google",
    "x-ai",
    "deepseek",
    "qwen",
    "moonshotai",
    "z-ai",
    "mistralai",
    "meta-llama",
];

/// The most OpenRouter entries the picker shows, the auto router included.
pub const CAP: usize = 16;
/// Kept per provider.
const PER_PROVIDER: usize = 2;
/// OpenRouter's auto router.
pub const AUTO: &str = "openrouter:openrouter/auto";

const FALLBACK: [(&str, &str); 9] = [
    (
        "anthropic/claude-sonnet-4.5",
        "Anthropic: Claude Sonnet 4.5",
    ),
    ("openai/gpt-5", "OpenAI: GPT-5"),
    ("google/gemini-2.5-pro", "Google: Gemini 2.5 Pro"),
    ("x-ai/grok-code-fast-1", "xAI: Grok Code Fast 1"),
    ("deepseek/deepseek-chat-v3.1", "DeepSeek: DeepSeek V3.1"),
    ("qwen/qwen3-coder", "Qwen: Qwen3 Coder 480B A35B"),
    ("moonshotai/kimi-k2-0905", "MoonshotAI: Kimi K2 0905"),
    ("z-ai/glm-4.6", "Z.AI: GLM 4.6"),
    ("mistralai/devstral-medium", "Mistral: Devstral Medium"),
];

fn choice(id: &str, label: &str) -> ModelChoice {
    ModelChoice {
        id: format!("openrouter:{id}"),
        label: label.to_string(),
        provider: "openrouter".into(),
        available: false,
    }
}

fn auto() -> ModelChoice {
    choice("openrouter/auto", "OpenRouter: Auto Router")
}

/// The static list used when there is no catalog.
pub fn fallback() -> Vec<ModelChoice> {
    std::iter::once(auto())
        .chain(FALLBACK.iter().map(|(id, label)| choice(id, label)))
        .collect()
}

fn variant(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    id.contains(':')
        || [
            "-preview", "beta", "audio", "image", "search", "-exp", "vision",
        ]
        .iter()
        .any(|v| id.contains(v))
}

/// The picker's OpenRouter choices from the catalog's JSON, or `None`
/// when it is not a catalog.
pub fn parse(json: &str) -> Option<Vec<ModelChoice>> {
    let v: Json = serde_json::from_str(json).ok()?;
    let data = v["data"].as_array()?;
    // (provider index, created, id, name)
    let mut groups: Vec<Vec<(f64, String, String)>> = vec![Vec::new(); PROVIDERS.len()];
    for m in data {
        let (Some(id), Some(name)) = (m["id"].as_str(), m["name"].as_str()) else {
            continue;
        };
        let tools = m["supported_parameters"]
            .as_array()
            .is_some_and(|p| p.iter().any(|p| p == "tools"));
        let provider = id.split('/').next().unwrap_or("");
        let Some(at) = PROVIDERS.iter().position(|p| *p == provider) else {
            continue;
        };
        if tools {
            let created = m["created"].as_f64().unwrap_or(0.0);
            groups[at].push((created, id.to_string(), name.to_string()));
        }
    }
    for group in &mut groups {
        if group.iter().any(|(_, id, _)| !variant(id)) {
            group.retain(|(_, id, _)| !variant(id));
        }
        group.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        group.truncate(PER_PROVIDER);
    }
    // Take the newest of every provider first, then the second ones, so
    // the cap trims depth before breadth; then show them grouped.
    let mut taken = vec![0usize; groups.len()];
    let mut left = CAP - 1;
    for depth in 0..PER_PROVIDER {
        for (g, group) in groups.iter().enumerate() {
            if left > 0 && group.len() > depth {
                taken[g] += 1;
                left -= 1;
            }
        }
    }
    let mut out = vec![auto()];
    for (g, group) in groups.iter().enumerate() {
        out.extend(
            group
                .iter()
                .take(taken[g])
                .map(|(_, id, name)| choice(id, name)),
        );
    }
    Some(out)
}

/// The cached catalog in `dir`, if one parses.
pub fn cached(dir: &Path) -> Option<Vec<ModelChoice>> {
    parse(&std::fs::read_to_string(dir.join("models.json")).ok()?)
}

/// Fetch the catalog from `base` (`…/api/v1`), and cache it in `dir`.
pub fn fetch(base: &str, dir: Option<&Path>) -> Result<Vec<ModelChoice>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let url = format!("{}/models", base.trim_end_matches('/'));
    let text = agent
        .get(&url)
        .call()
        .map_err(|e| format!("{url}: {e}"))?
        .into_body()
        .into_with_config()
        .limit(32 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| format!("{url}: {e}"))?;
    let models = parse(&text).ok_or_else(|| format!("{url}: not a model catalog"))?;
    if let Some(dir) = dir {
        if std::fs::create_dir_all(dir).is_ok() {
            let _ = std::fs::write(dir.join("models.json"), &text);
        }
    }
    Ok(models)
}

/// The model to choose when an OpenRouter key arrives: the newest Claude
/// Sonnet, else the newest Anthropic model, else the auto router.
pub fn default_model(choices: &[ModelChoice]) -> String {
    let ids = || choices.iter().map(|c| c.id.as_str());
    ids()
        .find(|id| id.starts_with("openrouter:anthropic/claude-sonnet"))
        .or_else(|| ids().find(|id| id.starts_with("openrouter:anthropic/")))
        .unwrap_or(AUTO)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/openrouter-models.json");

    #[test]
    fn the_catalog_keeps_tools_capable_current_models() {
        let choices = parse(FIXTURE).unwrap();
        let ids: Vec<&str> = choices
            .iter()
            .map(|c| c.id.trim_start_matches("openrouter:"))
            .collect();
        assert_eq!(
            ids,
            [
                "openrouter/auto",
                "anthropic/claude-sonnet-4.5",
                "anthropic/claude-opus-4.1",
                "openai/gpt-5",
                "openai/gpt-5-mini",
                "google/gemini-2.5-flash",
                "google/gemini-2.5-pro",
                "x-ai/grok-code-fast-1",
                "x-ai/grok-4",
                "deepseek/deepseek-chat-v3.1",
                "qwen/qwen3-max",
                "qwen/qwen3-next-80b-a3b-instruct",
                "moonshotai/kimi-k2-0905",
                "z-ai/glm-4.6",
                "mistralai/mistral-medium-3.1",
                "meta-llama/llama-4-maverick",
            ]
        );
        assert_eq!(choices.len(), CAP);
        assert_eq!(choices[1].label, "Anthropic: Claude Sonnet 4.5");
        assert!(choices.iter().all(|c| c.provider == "openrouter"));
        assert_eq!(
            default_model(&choices),
            "openrouter:anthropic/claude-sonnet-4.5"
        );
    }

    #[test]
    fn a_provider_with_only_variants_keeps_them() {
        let json = r#"{"data":[
          {"id":"z-ai/glm-5:free","name":"Z.AI: GLM 5 (free)","created":2,"supported_parameters":["tools"]},
          {"id":"z-ai/glm-5-preview","name":"Z.AI: GLM 5 Preview","created":1,"supported_parameters":["tools"]},
          {"id":"z-ai/glm-old","name":"no tools","created":3,"supported_parameters":[]}
        ]}"#;
        let ids: Vec<String> = parse(json).unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            [
                AUTO,
                "openrouter:z-ai/glm-5:free",
                "openrouter:z-ai/glm-5-preview"
            ]
        );
        assert!(parse("{\"nope\":1}").is_none());
        assert_eq!(
            default_model(&fallback()),
            "openrouter:anthropic/claude-sonnet-4.5"
        );
    }
}
