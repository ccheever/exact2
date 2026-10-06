//! Provider keys: the environment first, then `~/.config/exact-harness/keys`
//! (`KEY=VALUE` lines). A key saved with `setKey` is kept in memory and
//! written back to that file, mode 0600, other lines preserved. Keys never
//! leave this module except into a request's headers; `Debug` redacts them.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The names the file and the environment carry.
pub const NAMES: [&str; 4] = [
    "OPENROUTER_API_KEY",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OLLAMA_HOST",
];

/// The environment variable for a provider's key.
pub fn var_for(provider: &str) -> Option<&'static str> {
    match provider {
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        _ => None,
    }
}

/// The harness's configuration directory: `~/.config/exact-harness`.
pub fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    Some(Path::new(&home).join(".config").join("exact-harness"))
}

/// The keys in memory, and the directory their file lives in.
#[derive(Clone, Default)]
pub struct Keys {
    values: BTreeMap<String, String>,
    dir: Option<PathBuf>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("names", &self.values.keys().collect::<Vec<_>>())
            .finish()
    }
}

fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('#') {
                return None;
            }
            let (k, v) = l.strip_prefix("export ").unwrap_or(l).split_once('=')?;
            let v = v.trim().trim_matches(['"', '\'']).trim();
            (!v.is_empty()).then(|| (k.trim().to_string(), v.to_string()))
        })
        .collect()
}

impl Keys {
    /// Load: each name from the environment, else from the file in `dir`.
    pub fn load(dir: Option<PathBuf>) -> Keys {
        let file = dir
            .as_ref()
            .and_then(|d| std::fs::read_to_string(d.join("keys")).ok())
            .map(|t| parse(&t))
            .unwrap_or_default();
        let mut values = BTreeMap::new();
        for name in NAMES {
            let env = std::env::var(name).ok().filter(|v| !v.trim().is_empty());
            if let Some(v) = env.or_else(|| file.get(name).cloned()) {
                values.insert(name.to_string(), v.trim().to_string());
            }
        }
        Keys { values, dir }
    }

    /// The value of `name`, if set.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Whether `name` is set.
    pub fn has(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    /// Every value held, for scrubbing text that must not carry one.
    pub fn secrets(&self) -> Vec<String> {
        self.values
            .iter()
            .filter(|(k, v)| *k != "OLLAMA_HOST" && v.len() >= 6)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Set `name` (empty removes it) in memory and in the file. The error
    /// names the file, never the key.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() {
            self.values.remove(name);
        } else {
            self.values.insert(name.to_string(), value.to_string());
        }
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        write_line(dir, name, value)
    }
}

/// Rewrite `dir/keys` with `name` set to `value` (or removed when empty),
/// every other line kept, mode 0600.
pub fn write_line(dir: &Path, name: &str, value: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    let path = dir.join("keys");
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut placed = false;
    for line in old.lines() {
        let t = line.trim_start();
        let key = t.strip_prefix("export ").unwrap_or(t);
        let mine =
            key.split_once('=').is_some_and(|(k, _)| k.trim() == name) && !t.starts_with('#');
        if !mine {
            lines.push(line.to_string());
        } else if !placed && !value.is_empty() {
            lines.push(format!("{name}={value}"));
            placed = true;
        }
    }
    if !placed && !value.is_empty() {
        lines.push(format!("{name}={value}"));
    }
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    let fail = |e: std::io::Error| format!("cannot write {}: {e}", path.display());
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)
        .map_err(fail)?;
    // `mode` applies only to a file it creates; an existing one is set here.
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(fail)?;
    file.write_all(text.as_bytes()).map_err(fail)
}

/// `text` with every secret replaced by "[redacted]".
pub fn scrub(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for s in secrets {
        if !s.is_empty() && out.contains(s.as_str()) {
            out = out.replace(s.as_str(), "[redacted]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips_with_mode_0600_and_other_lines_kept() {
        let dir = std::env::temp_dir().join(format!("exact-harness-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keys"), "# mine\nFOO=bar\nOPENAI_API_KEY=old\n").unwrap();
        std::fs::set_permissions(dir.join("keys"), std::fs::Permissions::from_mode(0o644)).unwrap();
        let mut keys = Keys {
            values: BTreeMap::new(),
            dir: Some(dir.clone()),
        };
        keys.set("OPENROUTER_API_KEY", "  test-value-one  ")
            .unwrap();
        keys.set("OPENAI_API_KEY", "test-value-two").unwrap();
        let text = std::fs::read_to_string(dir.join("keys")).unwrap();
        assert_eq!(
            text,
            "# mine\nFOO=bar\nOPENAI_API_KEY=test-value-two\nOPENROUTER_API_KEY=test-value-one\n"
        );
        let mode = std::fs::metadata(dir.join("keys"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(keys.get("OPENROUTER_API_KEY"), Some("test-value-one"));
        assert!(!format!("{keys:?}").contains("test-value"));

        keys.set("OPENAI_API_KEY", "").unwrap();
        let text = std::fs::read_to_string(dir.join("keys")).unwrap();
        assert_eq!(text, "# mine\nFOO=bar\nOPENROUTER_API_KEY=test-value-one\n");
        assert!(!keys.has("OPENAI_API_KEY"));
        assert_eq!(
            parse(&text).get("OPENROUTER_API_KEY").map(String::as_str),
            Some("test-value-one")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scrub_hides_secrets() {
        let s = scrub("Bearer abcdef123 failed", &["abcdef123".into()]);
        assert_eq!(s, "Bearer [redacted] failed");
    }
}
