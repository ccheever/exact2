//! Hardened-runtime exception grants: one line admits one exception for one
//! executable that a macOS release signs, and the release derives its
//! `com.apple.security.cs.*` entitlement from the line.
//!
//! @ref LLP 1069.008 D9 (`runtime.<exception> <executable>`, scoped to the
//! app or to one executable under `host.macos.resources`)
//!
//! Like `device.*`, this is an exact2 grant: `io_grants` strips it before
//! `ibex2` parses the rest, and no runtime check reads it. The OS enforces
//! it: under the hardened runtime a process without the entitlement cannot
//! map JIT or unsigned executable memory, or load a library signed by
//! another team (Apple, "Hardened Runtime",
//! <https://developer.apple.com/documentation/security/hardened-runtime>).
//! The target is a path, not a key, because the entitlement belongs to the
//! one executable that needs it and to no other code in the bundle.

/// One hardened-runtime exception a grant can name. The name is Apple's
/// entitlement key without its `com.apple.security.cs.` prefix (the web has
/// no counterpart).
#[derive(Debug, PartialEq, Eq)]
pub struct Exception {
    /// The grant's name after `runtime.`.
    pub name: &'static str,
    /// The entitlement a release signs the target with.
    pub entitlement: &'static str,
}

/// Every exception grant. Adding a row is one entry here and no host code.
pub const EXCEPTIONS: &[Exception] = &[
    Exception {
        name: "allow-jit",
        entitlement: "com.apple.security.cs.allow-jit",
    },
    Exception {
        name: "allow-unsigned-executable-memory",
        entitlement: "com.apple.security.cs.allow-unsigned-executable-memory",
    },
    Exception {
        name: "disable-library-validation",
        entitlement: "com.apple.security.cs.disable-library-validation",
    },
];

/// The target naming the app's own executable; any other target is a path
/// under the bundle's `Contents`.
pub const APP: &str = "app";

/// One `runtime.<name> <target>` line.
#[derive(Debug, PartialEq, Eq)]
pub struct RuntimeGrant<'a> {
    /// The table's row.
    pub exception: &'static Exception,
    /// [`APP`], or the executable's path under `Contents`, inside a
    /// `host.macos.resources` destination (`Helpers/node/bin/node`).
    pub target: &'a str,
}

/// Whether a grant line is an exception grant (the runner's own family).
pub fn is_runtime_line(line: &str) -> bool {
    line.trim_start().starts_with("runtime.")
}

/// Whether `target` can name an executable a resources entry ships: a
/// relative path under `Resources/`, `Helpers/` or `Frameworks/`, the roots
/// `host.macos.resources` copies into, with no empty, `.` or `..` part.
fn helper_path(target: &str) -> bool {
    let mut parts = target.split('/');
    matches!(parts.next(), Some("Resources" | "Helpers" | "Frameworks"))
        && target.contains('/')
        && parts.all(|p| !p.is_empty() && p != "." && p != "..")
        && !target.contains(['\\', '\0', '"', '\''])
}

/// Every exception grant in `spec`, or every malformed one, each reported.
pub fn runtime_grants(spec: &str) -> Result<Vec<RuntimeGrant<'_>>, Vec<String>> {
    let (mut grants, mut errors) = (Vec::<RuntimeGrant>::new(), Vec::new());
    for line in spec.lines().map(str::trim).filter(|l| is_runtime_line(l)) {
        let mut words = line.split_whitespace();
        let name = words
            .next()
            .unwrap_or_default()
            .trim_start_matches("runtime.");
        let Some(exception) = EXCEPTIONS.iter().find(|e| e.name == name) else {
            let known: Vec<_> = EXCEPTIONS.iter().map(|e| e.name).collect();
            errors.push(format!(
                "`runtime.{name}` is not a runtime exception; the table has {}",
                known.join(", ")
            ));
            continue;
        };
        let (Some(target), None) = (words.next(), words.next()) else {
            errors.push(format!(
                "`{line}`: a runtime exception names one executable, `{APP}` or its path under Contents, as in `runtime.{name} Helpers/node/bin/node`"
            ));
            continue;
        };
        if target != APP && !helper_path(target) {
            errors.push(format!(
                "`{line}`: the target is `{APP}` or a path under Resources/, Helpers/ or Frameworks/ inside a `host.macos.resources` destination"
            ));
            continue;
        }
        let grant = RuntimeGrant { exception, target };
        if !grants.contains(&grant) {
            grants.push(grant);
        }
    }
    if errors.is_empty() {
        Ok(grants)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_runtime_line_names_a_row_and_one_executable() {
        let spec = "net.fetch https://x/\nruntime.allow-jit Helpers/node/bin/node\nruntime.disable-library-validation app\nruntime.allow-jit Helpers/node/bin/node";
        let all = runtime_grants(spec).unwrap();
        assert_eq!(
            all.iter()
                .map(|g| (g.exception.entitlement, g.target))
                .collect::<Vec<_>>(),
            [
                ("com.apple.security.cs.allow-jit", "Helpers/node/bin/node"),
                ("com.apple.security.cs.disable-library-validation", "app")
            ]
        );
        let errors = runtime_grants(
            "runtime.debugger app\nruntime.allow-jit\nruntime.allow-jit app extra\nruntime.allow-jit MacOS/Exact\nruntime.allow-jit Helpers/../MacOS/Exact\nruntime.allow-jit Helpers\nruntime.allow-jit /Helpers/x",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 7, "{errors:?}");
        assert!(errors[0].contains("not a runtime exception"));
        assert!(errors[1..3]
            .iter()
            .all(|e| e.contains("names one executable")));
        assert!(errors[3..].iter().all(|e| e.contains("the target is")));
    }
}
