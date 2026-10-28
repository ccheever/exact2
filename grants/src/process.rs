//! The desktop-process lines (LLP 1133.007 §6): `process.spawn <target>`,
//! `env.read login:<NAME>` and `secret.hold <name>`. They are exact2's own
//! lines: the runner reads `secret.hold` and the data executor admits a
//! program and a login variable from them, and the I/O grammar of
//! [`crate::GrantSet`] never sees them.
use crate::{valid_name, PathPrefix, PRESENCE_PREFIX};

/// The capability that names one program (D7).
pub const SPAWN: &str = "process.spawn";
/// The prefix of a login shell variable's `env.read` target (D5).
pub const LOGIN: &str = "login:";
/// The capability that names a field's held secret (D8).
pub const HOLD: &str = "secret.hold";

/// What a `process.spawn` line names (D7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnTarget {
    /// `path:<name>`: the first executable of that name on a `PATH`.
    Name(String),
    /// A bundled helper (`bundle:/…`), an absolute path, or a home or app
    /// path: that file, or any executable under it, by whole names.
    Path(PathPrefix),
}

impl SpawnTarget {
    /// Read one line's target.
    pub fn parse(target: &str) -> Result<Self, String> {
        if let Some(name) = target.strip_prefix("path:") {
            let bad = name.is_empty()
                || matches!(name, "." | "..")
                || name
                    .chars()
                    .any(|c| c == '/' || c == '\\' || c.is_control());
            return if bad {
                Err(format!("`{target}`: `path:` takes a program's name alone"))
            } else {
                Ok(SpawnTarget::Name(name.to_string()))
            };
        }
        if target.starts_with("doc:/") {
            return Err(format!("`{target}`: a chosen document is not a program"));
        }
        let prefix = PathPrefix::new(target).ok_or_else(|| {
            format!("`{target}` is `path:<name>`, a `bundle:`, `home:` or `app:` path, or an absolute one")
        })?;
        if prefix.components().len() == 1 && prefix.components()[0].ends_with(':') {
            return Err(format!(
                "`{target}`: a line names a program or a directory of them, not a whole namespace"
            ));
        }
        Ok(SpawnTarget::Path(prefix))
    }

    /// Whether a call's program, written as a line's target is
    /// (`path:ssh`, `home:/.t3/runtime/versions/1.2.0/t3`), is this one's.
    pub fn admits(&self, program: &str) -> bool {
        match self {
            SpawnTarget::Name(name) => program.strip_prefix("path:") == Some(name.as_str()),
            SpawnTarget::Path(prefix) => !program.starts_with("path:") && prefix.covers(program),
        }
    }
}

/// Whether a trimmed line is one of these.
pub fn is_process_line(line: &str) -> bool {
    let mut words = line.split_whitespace();
    match words.next() {
        Some(SPAWN | HOLD) => true,
        Some("env.read") => words.next().is_some_and(|t| t.starts_with(LOGIN)),
        _ => false,
    }
}

/// Read one of these lines whole: `Ok` when it is well formed.
pub fn validate(line: &str) -> Result<(), String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let [capability, target] = words[..] else {
        return Err(format!("`{line}`: one capability and one target"));
    };
    match capability {
        SPAWN => SpawnTarget::parse(target).map(|_| ()),
        HOLD if target.starts_with(PRESENCE_PREFIX) => Err(format!(
            "`{target}`: `{PRESENCE_PREFIX}` names are the runner's own"
        )),
        HOLD if !valid_name(target) => Err(format!(
            "`{target}` is not a secret name ([a-z0-9._-]{{1,64}})"
        )),
        HOLD => Ok(()),
        _ => {
            let name = target.strip_prefix(LOGIN).unwrap_or_default();
            if is_variable(name) {
                Ok(())
            } else {
                Err(format!("`{target}`: `login:` takes one variable's name"))
            }
        }
    }
}

/// A POSIX environment variable's portable name.
fn is_variable(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Each line of `spec` with `capability`, by its target; one of these
/// lines only when it is well formed.
fn targets<'a>(spec: &'a str, capability: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    spec.lines().filter_map(move |line| {
        let line = line.trim();
        let mut words = line.split_whitespace();
        let ours = is_process_line(line);
        (words.next() == Some(capability) && (!ours || validate(line).is_ok()))
            .then(|| words.next())
            .flatten()
    })
}

/// The `process.spawn` line of `spec` that admits `program`, if one does.
pub fn admitting<'a>(spec: &'a str, program: &str) -> Option<&'a str> {
    spec.lines().map(str::trim).find(|line| {
        line.split_whitespace().next() == Some(SPAWN)
            && validate(line).is_ok()
            && line
                .split_whitespace()
                .nth(1)
                .and_then(|t| SpawnTarget::parse(t).ok())
                .is_some_and(|t| t.admits(program))
    })
}

/// Whether `spec` has a `process.spawn` line: `processStatus().state`.
pub fn spawns(spec: &str) -> bool {
    targets(spec, SPAWN).next().is_some()
}

/// The login shell's variables `spec` names, in declaration order (D5).
pub fn login_variables(spec: &str) -> Vec<&str> {
    let mut names: Vec<&str> = targets(spec, "env.read")
        .filter_map(|t| t.strip_prefix(LOGIN))
        .collect();
    names.dedup();
    names
}

/// The names `spec` holds with `secret.hold`, in declaration order (D8).
pub fn held_secrets(spec: &str) -> Vec<&str> {
    targets(spec, HOLD).collect()
}

/// A name both kept and held, which `spec` refuses (`secret-name-twice`).
pub fn named_twice(spec: &str) -> Option<String> {
    let kept: Vec<&str> = targets(spec, "secret.keep").collect();
    held_secrets(spec).into_iter().find(|h| kept.contains(h)).map(|name| {
        format!("secret-name-twice: `{name}` is both `secret.keep` and `secret.hold`; a name is kept or held")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_targets_name_a_program_by_whole_names() {
        let line = SpawnTarget::parse("home:/.t3/runtime/versions").unwrap();
        assert!(line.admits("home:/.t3/runtime/versions/1.2.0/t3"));
        assert!(!line.admits("home:/.t3/runtime/versions2/t3"));
        assert!(!line.admits("path:versions"));
        let ssh = SpawnTarget::parse("path:ssh").unwrap();
        assert!(ssh.admits("path:ssh"));
        assert!(!ssh.admits("/usr/bin/ssh"));
        assert!(SpawnTarget::parse("/usr/bin/tar")
            .unwrap()
            .admits("/usr/bin/tar"));
        for bad in [
            "path:",
            "path:a/b",
            "home:/",
            "bundle:/",
            "doc:/x",
            "ssh",
            "app:/../x",
        ] {
            assert!(SpawnTarget::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn lines_are_read_whole() {
        for good in [
            "process.spawn path:tailscale",
            "process.spawn bundle:/Helpers/t3/bin/t3",
            "env.read login:PATH",
            "env.read login:SSH_AUTH_SOCK",
            "secret.hold ssh.password",
        ] {
            assert!(is_process_line(good), "{good}");
            validate(good).unwrap();
        }
        for bad in [
            "process.spawn path:ssh extra",
            "env.read login:",
            "env.read login:1PATH",
            "secret.hold Bad",
            "secret.hold exact.presence.x",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
        assert!(!is_process_line("env.read PATH"));
    }

    #[test]
    fn a_name_is_kept_or_held() {
        let spec = "secret.keep token\nsecret.hold ssh.password\nenv.read login:PATH";
        assert_eq!(named_twice(spec), None);
        assert_eq!(held_secrets(spec), ["ssh.password"]);
        assert_eq!(login_variables(spec), ["PATH"]);
        let twice = "secret.keep ssh.password\nsecret.hold ssh.password";
        assert!(named_twice(twice).unwrap().starts_with("secret-name-twice"));
        let spec = "process.spawn path:ssh\nprocess.spawn home:/.t3/runtime";
        assert_eq!(
            admitting(spec, "home:/.t3/runtime/t3"),
            Some("process.spawn home:/.t3/runtime")
        );
        assert_eq!(admitting(spec, "path:tar"), None);
        assert!(spawns(spec) && !spawns("env.read login:PATH"));
    }
}
