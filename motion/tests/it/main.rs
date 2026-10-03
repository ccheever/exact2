//! Motion's integration tests: one binary, so one link and one launch.

mod animation;
mod cadence;
mod clock;
mod descriptor;
mod easing;
mod engine;
mod height;
mod hold;
mod paint;
mod presence;
mod spring;
mod transform_hold;

use exact_motion::{Animations, Keyframes};

/// A shorthand then the `@keyframes name{…}` rules it names, resolved as a
/// runner resolves a row against its plan's table (LLP 1055 D5).
pub fn keyframed(text: &str) -> Result<Animations, String> {
    let (head, rules) = text.split_at(text.find("@keyframes").unwrap_or(text.len()));
    let mut table = Vec::new();
    for rule in rules.split("@keyframes").skip(1) {
        let open = rule.find('{').ok_or("a rule")?;
        let body = rule[open + 1..]
            .trim_end()
            .strip_suffix('}')
            .ok_or("a body")?;
        let k = Keyframes::parse(body).map_err(|e| format!("{e:?}"))?;
        table.push((rule[..open].trim().to_string(), k));
    }
    let mut a = Animations::parse(head).map_err(|e| format!("{e:?}"))?;
    let dropped = a.resolve(|n| table.iter().find(|(m, _)| m == n).map(|(_, k)| k));
    if dropped.is_empty() {
        Ok(a)
    } else {
        Err(format!("unknown {dropped:?}"))
    }
}
