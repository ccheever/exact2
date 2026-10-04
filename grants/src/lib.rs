//! The authority model: carried, not inferred.
//!
//! LLP 0067 R2 binds a capability grant to the identity of the binding a
//! module was handed at instantiation. Nothing here inspects a call stack,
//! because nothing in Ibex 2 is permitted to.
//!
//! Under D1 and D2 the binary question — *may this module reach the network at
//! all?* — is answered structurally: a module that was not injected `fetch`
//! has no expression that evaluates to one. So what lives here is only the
//! **parameterized** half of LLP 0059.000 §4: the per-family questions asked
//! against a grant the caller already provably holds (LLP 0067 §2 keeps the
//! current list).
//!
//! This is pure Rust and needs no engine, which is why it is the first thing
//! built.
//!
//! @ref LLP 0067#3-the-check — the parameterized questions the boundary asks
//! @ref LLP 0059.000#4-capability-summary — the six capabilities and their granularity

use std::collections::BTreeSet;

/// An origin, for the two network capabilities.
///
/// Compared by exact tuple equality. The one pattern is Exact's patch 1
/// (`EXACT-PATCHES.md`): a `net.fetch` grant may name every host under one
/// domain, `https://*.host.example`, and nothing wider. It is its own grant
/// kind, `FetchSubdomains`, so an exact grant never matches as a pattern.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl Origin {
    pub fn new(scheme: &str, host: &str, port: u16) -> Self {
        // Scheme and host are case-insensitive per RFC 3986; normalize once at
        // construction so comparison is a plain equality test everywhere else.
        Self {
            scheme: scheme.to_ascii_lowercase(),
            host: host.to_ascii_lowercase(),
            port,
        }
    }
}

/// A filesystem subtree, for `fs.read` and `fs.write`.
///
/// Paths are compared by **whole components**. A prefix grant on `/home/user`
/// must not admit `/home/user2`, which a naive string `starts_with` would —
/// that is the standard way a per-prefix grant turns into a wider one than
/// anybody wrote down.
///
/// Paths reaching here are expected to be already resolved through the virtual
/// filesystem namespace (LLP 0059.000 §3.11), so `..` has no meaning at this
/// layer and is rejected rather than interpreted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathPrefix(Vec<String>);

impl PathPrefix {
    /// Build a prefix from an absolute, already-resolved path.
    ///
    /// Returns `None` for a relative path or one still carrying `.`/`..`,
    /// because admitting either would mean deciding traversal semantics here,
    /// where the answer cannot be checked against the real filesystem.
    pub fn new(path: &str) -> Option<Self> {
        // `doc:/` names the documents a person chose (Exact patch: LLP
        // 1069.010 D1), a namespace beside `app:/` resolved by the host.
        let (namespace, path) = if let Some(path) = path.strip_prefix("app:/") {
            ("app:", path)
        } else if let Some(path) = path.strip_prefix("doc:/") {
            ("doc:", path)
        } else if path.starts_with('/') {
            ("", path)
        } else {
            return None;
        };
        let mut components = vec![namespace.to_string()];
        for component in path.split('/') {
            match component {
                "" => continue,
                "." | ".." => return None,
                other => components.push(other.to_string()),
            }
        }
        Some(Self(components))
    }

    /// Does this prefix cover `path`?
    ///
    /// A prefix covers itself and everything beneath it. An unresolvable path
    /// is covered by nothing — refusing is the only safe reading of a path
    /// this layer cannot interpret.
    pub fn covers(&self, path: &str) -> bool {
        match Self::new(path) {
            Some(target) => target.0.starts_with(&self.0),
            None => false,
        }
    }

    /// The canonical namespace and whole path components. Browser hosts use
    /// this parsed form directly; they never re-read the source grammar.
    pub fn components(&self) -> &[String] {
        &self.0
    }
}

/// One operation the boundary is asked to admit.
///
/// Every variant carries the parameter its grant is scoped by. There is no
/// unparameterized variant, and adding one would be the moment per-origin and
/// per-prefix stopped meaning anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// `fetch` to an origin. Capability `net.fetch`.
    Fetch { origin: Origin },
    /// A WebSocket to an origin. Capability `net.websocket`.
    WebSocket { origin: Origin },
    /// Read a path. Capability `fs.read`.
    FsRead { path: String },
    /// Write a path. Capability `fs.write`.
    FsWrite { path: String },
    /// Read one environment variable by name. Capability `env.read`.
    EnvRead { name: String },
    /// Open a database by path. Capability `sqlite.open`.
    SqliteOpen { path: String },
    /// Keep — read, replace, forget — one secret by name. Capability
    /// `secret.keep` (LLP 0069 §1).
    SecretKeep { name: String },
    /// Read, write, delete, and list durable state under one scope.
    /// Capability `storage.kv` (LLP 0070 §1): keys are free within the
    /// scope, and the scope is the grant.
    StorageKv { scope: String },
}

/// One grant. The parameter is the scope, and there is always a scope.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Grant {
    Fetch(Origin),
    /// `net.fetch https://*.<suffix>`: every host strictly under `suffix`, at
    /// the origin's scheme and port (Exact patch 1). The suffix is at least
    /// two labels and not an address.
    FetchSubdomains(Origin),
    WebSocket(Origin),
    FsRead(PathPrefix),
    FsWrite(PathPrefix),
    EnvRead(String),
    SqliteOpen(PathPrefix),
    SecretKeep(String),
    StorageKv(String),
}

impl Grant {
    fn admits(&self, operation: &Operation) -> bool {
        match (self, operation) {
            (Grant::Fetch(granted), Operation::Fetch { origin }) => granted == origin,
            (Grant::FetchSubdomains(suffix), Operation::Fetch { origin }) => {
                origin.scheme == suffix.scheme
                    && origin.port == suffix.port
                    && origin
                        .host
                        .strip_suffix(suffix.host.as_str())
                        .is_some_and(|head| head.len() > 1 && head.ends_with('.'))
            }
            (Grant::WebSocket(granted), Operation::WebSocket { origin }) => granted == origin,
            (Grant::FsRead(prefix), Operation::FsRead { path }) => prefix.covers(path),
            (Grant::FsWrite(prefix), Operation::FsWrite { path }) => prefix.covers(path),
            (Grant::EnvRead(granted), Operation::EnvRead { name }) => granted == name,
            (Grant::SqliteOpen(prefix), Operation::SqliteOpen { path }) => prefix.covers(path),
            (Grant::SecretKeep(granted), Operation::SecretKeep { name }) => granted == name,
            (Grant::StorageKv(granted), Operation::StorageKv { scope }) => granted == scope,
            // Cross-kind pairs are not merely false, they are the whole point:
            // an `fs.read` grant admits no network operation, and the match
            // above is exhaustive over kinds so a new capability cannot be
            // added without deciding what admits it.
            _ => false,
        }
    }
}

/// The authority a single binding carries.
///
/// This is the value LLP 0060 D1 is about. It is handed to a binding at module
/// instantiation and consulted at the boundary. It is never derived from a
/// frame, a `Domain`, or a job queue, and there is deliberately no constructor
/// that reads ambient state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct GrantSet {
    grants: BTreeSet<Grant>,
}

impl GrantSet {
    /// The empty set: holds nothing, admits nothing.
    pub fn none() -> Self {
        Self::default()
    }

    pub fn with(mut self, grant: Grant) -> Self {
        self.grants.insert(grant);
        self
    }

    /// The environment variables this set may read, in a stable order.
    ///
    /// `process.env` is a **snapshot**, not a live proxy (LLP 0059.000 §3.8),
    /// and the snapshot is built from exactly this list — so a module cannot
    /// see a variable it was not granted, with no check at read time. The
    /// capability is the object's contents.
    /// This set with every filesystem prefix realized — symlinks followed,
    /// case settled — for checking a realized request path against
    /// (`stdlib::fs::realize`). Other families are unchanged.
    pub fn map_fs(&self, resolve: impl Fn(&str) -> String) -> GrantSet {
        let realize = |prefix: &PathPrefix| {
            if prefix.0.first().is_some_and(|p| p == "app:" || p == "doc:") {
                return prefix.clone();
            }
            let spelt = format!("/{}", prefix.0[1..].join("/"));
            let real = resolve(&spelt);
            PathPrefix::new(&real).unwrap_or_else(|| prefix.clone())
        };
        GrantSet {
            grants: self
                .grants
                .iter()
                .map(|grant| match grant {
                    Grant::FsRead(prefix) => Grant::FsRead(realize(prefix)),
                    Grant::FsWrite(prefix) => Grant::FsWrite(realize(prefix)),
                    Grant::SqliteOpen(prefix) => Grant::SqliteOpen(realize(prefix)),
                    other => other.clone(),
                })
                .collect(),
        }
    }

    /// The kv scopes this set may use, in a stable order (LLP 0070 §1).
    pub fn kv_scopes(&self) -> Vec<&str> {
        self.grants
            .iter()
            .filter_map(|grant| match grant {
                Grant::StorageKv(scope) => Some(scope.as_str()),
                _ => None,
            })
            .collect()
    }

    /// The secrets this set may keep, in a stable order: the load list a
    /// host reads into a snapshot before its consumer boots (LLP 0069 §1).
    pub fn kept_secrets(&self) -> Vec<&str> {
        self.grants
            .iter()
            .filter_map(|grant| match grant {
                Grant::SecretKeep(name) => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    pub fn readable_env(&self) -> Vec<&str> {
        self.grants
            .iter()
            .filter_map(|grant| match grant {
                Grant::EnvRead(name) => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Is `operation` admitted by any grant in this set?
    ///
    /// Fails closed: an empty set admits nothing, and no operation has a
    /// default-allow path.
    pub fn permits(&self, operation: &Operation) -> bool {
        self.grants.iter().any(|grant| grant.admits(operation))
    }

    pub fn is_empty(&self) -> bool {
        self.grants.is_empty()
    }

    /// The parsed grants in stable order. Hosts that serialize authority for
    /// another runtime match these typed values instead of parsing source text.
    pub fn iter(&self) -> impl Iterator<Item = &Grant> {
        self.grants.iter()
    }
}

impl GrantSet {
    /// Parse a grant spec: one grant per line, `capability target`.
    ///
    /// Fixed at creation, because a grant set that can be added to after the
    /// fact is ambient authority wearing a struct (LLP 0067 §2). Blank lines
    /// and `#` comments are ignored.
    ///
    /// ```text
    /// net.fetch http://127.0.0.1:8080
    /// fs.read   /data
    /// env.read  NODE_ENV
    /// ```
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut set = GrantSet::none();
        for (index, line) in spec.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let capability = parts.next().unwrap_or_default();
            let target = parts
                .next()
                .ok_or_else(|| format!("line {}: `{capability}` needs a target", index + 1))?;
            if matches!(capability, "net.fetch" | "net.websocket") && target.contains('@') {
                // `https://api.example.com@evil.com` is the origin `evil.com`:
                // a line that reads as one host and admits another is refused
                // here, before either parser (native or the browser's) sees it.
                return Err(format!(
                    "line {}: `{target}`: an origin has no user or password",
                    index + 1
                ));
            }
            let grant = match capability {
                "net.fetch" if target.contains("://*.") => {
                    Grant::FetchSubdomains(subdomains(target).map_err(|e| {
                        format!("line {}: bad origin pattern `{target}`: {e}", index + 1)
                    })?)
                }
                "net.fetch" | "net.websocket" => {
                    if target.contains('*') {
                        return Err(format!(
                            "line {}: `{target}`: `*` is allowed only as `net.fetch scheme://*.domain`",
                            index + 1
                        ));
                    }
                    let origin = origin(target, false)
                        .map_err(|e| format!("line {}: bad origin `{target}`: {e}", index + 1))?;
                    if capability == "net.fetch" {
                        Grant::Fetch(origin)
                    } else {
                        Grant::WebSocket(origin)
                    }
                }
                "fs.read" | "fs.write" | "sqlite.open" => {
                    let prefix = PathPrefix::new(target).ok_or_else(|| {
                        format!(
                            "line {}: `{target}` must be an absolute, resolved path",
                            index + 1
                        )
                    })?;
                    match capability {
                        "fs.read" => Grant::FsRead(prefix),
                        "fs.write" => Grant::FsWrite(prefix),
                        _ => Grant::SqliteOpen(prefix),
                    }
                }
                "env.read" => Grant::EnvRead(target.to_string()),
                "secret.keep" => {
                    if !valid_name(target) {
                        return Err(format!(
                            "line {}: `{target}` is not a secret name ([a-z0-9._-]{{1,64}})",
                            index + 1
                        ));
                    }
                    Grant::SecretKeep(target.to_string())
                }
                "storage.kv" => {
                    if !valid_name(target) {
                        return Err(format!(
                            "line {}: `{target}` is not a kv scope ([a-z0-9._-]{{1,64}})",
                            index + 1
                        ));
                    }
                    Grant::StorageKv(target.to_string())
                }
                other => return Err(format!("line {}: unknown capability `{other}`", index + 1)),
            };
            // One capability and one target per line. Anything after the
            // target refuses the line: a manifest typo that split a target
            // must stop a deployment, not silently grant its first word.
            if let Some(extra) = parts.next() {
                return Err(format!(
                    "line {}: unexpected `{extra}` after the target",
                    index + 1
                ));
            }
            set = set.with(grant);
        }
        Ok(set)
    }
}

/// The suffix origin of `scheme://*.suffix[:port]` (Exact patch 1): `*` is
/// the whole leftmost label and appears nowhere else, the suffix is a domain
/// of two labels or more (`*.com` would be every site), and nothing follows
/// the origin.
fn subdomains(target: &str) -> Result<Origin, String> {
    let (scheme, rest) = target
        .split_once("://*.")
        .ok_or("expected `scheme://*.domain`")?;
    if rest.contains('*') || scheme.contains('*') {
        return Err("`*` is allowed only as the whole leftmost label".into());
    }
    origin(&format!("{scheme}://{rest}"), true)
}

// Native uses url's WHATWG parser; wasm asks its browser executor for the
// same normalization. ICU tables belong to that executor, not each app.
#[cfg(not(target_arch = "wasm32"))]
fn origin(target: &str, wildcard: bool) -> Result<Origin, String> {
    let url = url::Url::parse(target).map_err(|e| e.to_string())?;
    let host = url.host_str().ok_or("origin has no host")?;
    if wildcard {
        if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
            return Err("an origin has no path, query or fragment".into());
        }
        if !matches!(url.host(), Some(url::Host::Domain(_))) {
            return Err("the suffix must be a domain, not an address".into());
        }
        if host.split('.').filter(|label| !label.is_empty()).count() < 2 || host.ends_with('.') {
            return Err(
                "the suffix needs at least two labels (`*.example.com`, not `*.com`)".into(),
            );
        }
    }
    let port = url
        .port_or_known_default()
        .ok_or_else(|| format!("no default port for `{}`", url.scheme()))?;
    Ok(Origin::new(url.scheme(), host, port))
}

#[cfg(target_arch = "wasm32")]
fn origin(target: &str, wildcard: bool) -> Result<Origin, String> {
    #[link(wasm_import_module = "exact_grants")]
    unsafe extern "C" {
        fn origin(
            input: *const u8,
            len: usize,
            wildcard: u32,
            output: *mut u8,
            capacity: usize,
        ) -> i32;
    }
    // The executor writes scheme, host and port separated by NUL. A negative
    // length refuses; a length-only call never borrows memory across growth.
    let size = unsafe {
        origin(
            target.as_ptr(),
            target.len(),
            wildcard.into(),
            std::ptr::null_mut(),
            0,
        )
    };
    if size < 1 {
        return Err("invalid URL origin".into());
    }
    let mut output = vec![0; size as usize];
    let copied = unsafe {
        origin(
            target.as_ptr(),
            target.len(),
            wildcard.into(),
            output.as_mut_ptr(),
            output.len(),
        )
    };
    if copied != size {
        return Err("invalid URL origin".into());
    }
    let text = std::str::from_utf8(&output).map_err(|_| "invalid URL origin")?;
    let mut parts = text.split('\0');
    let scheme = parts.next().ok_or("invalid URL origin")?;
    let host = parts.next().ok_or("invalid URL origin")?;
    let port = parts
        .next()
        .ok_or("invalid URL origin")?
        .parse()
        .map_err(|_| "invalid URL port")?;
    Ok(Origin::new(scheme, host, port))
}

/// Secret and storage scope names (ibex LLP 0069 / 0070).
pub fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
        && !name.bytes().all(|b| b == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(host: &str) -> Origin {
        Origin::new("https", host, 443)
    }

    #[test]
    fn empty_set_admits_nothing() {
        let set = GrantSet::none();
        assert!(!set.permits(&Operation::Fetch {
            origin: origin("example.com")
        }));
        assert!(!set.permits(&Operation::EnvRead {
            name: "PATH".into()
        }));
        assert!(!set.permits(&Operation::FsRead {
            path: "/tmp/x".into()
        }));
    }

    #[test]
    fn fetch_grant_is_per_origin() {
        let set = GrantSet::none().with(Grant::Fetch(origin("api.example.com")));
        assert!(set.permits(&Operation::Fetch {
            origin: origin("api.example.com")
        }));
        assert!(!set.permits(&Operation::Fetch {
            origin: origin("evil.example.com")
        }));
        // A different port is a different origin.
        assert!(!set.permits(&Operation::Fetch {
            origin: Origin::new("https", "api.example.com", 8443)
        }));
        // ...and a different scheme is too.
        assert!(!set.permits(&Operation::Fetch {
            origin: Origin::new("http", "api.example.com", 443)
        }));
    }

    #[test]
    fn origin_comparison_is_case_insensitive_on_scheme_and_host() {
        let set = GrantSet::none().with(Grant::Fetch(Origin::new("HTTPS", "API.Example.COM", 443)));
        assert!(set.permits(&Operation::Fetch {
            origin: origin("api.example.com")
        }));
    }

    #[test]
    fn a_subdomain_grant_admits_hosts_strictly_under_its_domain() {
        let set = GrantSet::parse("net.fetch https://*.host.bsky.network").unwrap();
        let fetch = |origin: Origin| set.permits(&Operation::Fetch { origin });
        assert!(fetch(origin("morel.us-east.host.bsky.network")));
        assert!(fetch(origin("a.host.bsky.network")));
        assert!(fetch(origin("A.Host.Bsky.Network")));
        // Not the domain itself, not a lookalike, not another scheme or port.
        assert!(!fetch(origin("host.bsky.network")));
        assert!(!fetch(origin("evilhost.bsky.network")));
        assert!(!fetch(origin("a.host.bsky.network.evil.com")));
        assert!(!fetch(Origin::new("http", "a.host.bsky.network", 443)));
        assert!(!fetch(Origin::new("https", "a.host.bsky.network", 8443)));
        // Fetch only.
        assert!(!set.permits(&Operation::WebSocket {
            origin: origin("a.host.bsky.network")
        }));
    }

    #[test]
    fn a_subdomain_pattern_is_refused_unless_it_names_one_domain() {
        for bad in [
            "net.fetch https://*.com",
            "net.fetch https://a.*.example.com",
            "net.fetch https://*.*.example.com",
            "net.fetch https://*.127.0.0.1",
            "net.fetch https://*.example.com/path",
            "net.fetch https://*example.com",
            "net.websocket wss://*.example.com",
            "net.fetch https://*.example.com@evil.com",
            "net.fetch https://*.example.com:pw@evil.com",
        ] {
            assert!(GrantSet::parse(bad).is_err(), "{bad} parsed");
        }
        assert!(GrantSet::parse("net.fetch https://*.example.com:8443").is_ok());
    }

    #[test]
    fn an_origin_spelt_with_userinfo_is_refused() {
        for bad in [
            "net.fetch https://api.example.com@evil.com",
            "net.fetch https://user:pw@api.example.com",
            "net.websocket wss://api.example.com@evil.com",
        ] {
            assert!(GrantSet::parse(bad).is_err(), "{bad} parsed");
        }
    }

    #[test]
    fn a_fetch_grant_does_not_admit_a_websocket() {
        let set = GrantSet::none().with(Grant::Fetch(origin("api.example.com")));
        assert!(!set.permits(&Operation::WebSocket {
            origin: origin("api.example.com")
        }));
    }

    #[test]
    fn path_prefix_matches_whole_components_only() {
        let prefix = PathPrefix::new("/home/user").expect("absolute");
        assert!(prefix.covers("/home/user"));
        assert!(prefix.covers("/home/user/notes.txt"));
        assert!(prefix.covers("/home/user/deep/nested/file"));
        // The bug this test exists for: a string prefix would admit these.
        assert!(!prefix.covers("/home/user2"));
        assert!(!prefix.covers("/home/username/secrets"));
        assert!(!prefix.covers("/home"));
        assert!(!prefix.covers("/etc/passwd"));
    }

    #[test]
    fn unresolved_and_relative_paths_are_admitted_by_nothing() {
        let prefix = PathPrefix::new("/home/user").expect("absolute");
        assert!(!prefix.covers("/home/user/../../etc/passwd"));
        assert!(!prefix.covers("/home/user/./notes.txt"));
        assert!(!prefix.covers("home/user/notes.txt"));
        assert_eq!(PathPrefix::new("relative/path"), None);
        assert_eq!(PathPrefix::new("/home/../etc"), None);
    }

    #[test]
    fn redundant_separators_do_not_change_coverage() {
        let prefix = PathPrefix::new("/home//user/").expect("absolute");
        assert!(prefix.covers("/home/user/notes.txt"));
        assert!(!prefix.covers("/home/user2"));
    }

    #[test]
    fn read_and_write_are_separate_grants() {
        let set = GrantSet::none().with(Grant::FsRead(PathPrefix::new("/data").unwrap()));
        assert!(set.permits(&Operation::FsRead {
            path: "/data/x".into()
        }));
        assert!(!set.permits(&Operation::FsWrite {
            path: "/data/x".into()
        }));
    }

    #[test]
    fn env_read_is_per_variable_name() {
        let set = GrantSet::none().with(Grant::EnvRead("NODE_ENV".into()));
        assert!(set.permits(&Operation::EnvRead {
            name: "NODE_ENV".into()
        }));
        // The canonical supply-chain case from LLP 0059.000 §3.8.
        assert!(!set.permits(&Operation::EnvRead {
            name: "AWS_SECRET_ACCESS_KEY".into()
        }));
        // Names are compared exactly; no prefix or case folding.
        assert!(!set.permits(&Operation::EnvRead {
            name: "node_env".into()
        }));
        assert!(!set.permits(&Operation::EnvRead {
            name: "NODE_ENV_EXTRA".into()
        }));
    }

    #[test]
    fn secret_keep_is_per_name_and_crosses_no_kind() {
        let set = GrantSet::parse("secret.keep castle.session\nenv.read HOME\n").unwrap();
        assert!(set.permits(&Operation::SecretKeep {
            name: "castle.session".into()
        }));
        assert!(!set.permits(&Operation::SecretKeep {
            name: "castle.session.old".into()
        }));
        assert!(!set.permits(&Operation::SecretKeep {
            name: "HOME".into()
        }));
        assert!(!set.permits(&Operation::EnvRead {
            name: "castle.session".into()
        }));
        assert_eq!(set.kept_secrets(), vec!["castle.session"]);
        assert!(GrantSet::parse("secret.keep ../x").is_err());
        assert!(GrantSet::parse("secret.keep").is_err());
    }

    #[test]
    fn storage_kv_is_per_scope() {
        let set = GrantSet::parse("storage.kv castle.state\n").unwrap();
        assert_eq!(set.kv_scopes(), vec!["castle.state"]);
        assert!(set.permits(&Operation::StorageKv {
            scope: "castle.state".into()
        }));
        // Neither direction of a prefix admits the other.
        assert!(!set.permits(&Operation::StorageKv {
            scope: "castle.state.old".into()
        }));
        assert!(!GrantSet::parse("storage.kv castle\n")
            .unwrap()
            .permits(&Operation::StorageKv {
                scope: "castle.state".into()
            }));
        for bad in [
            "storage.kv ../x",
            "storage.kv .",
            "storage.kv ..",
            "storage.kv ...",
            "storage.kv Castle.state",
            "storage.kv",
        ] {
            assert!(GrantSet::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn storage_kv_crosses_no_kind_in_either_direction() {
        // Single-family sets, so each assertion can actually witness the
        // isolation: a combined set would admit both operations and prove
        // nothing.
        let kv_only = GrantSet::parse("storage.kv castle.state\n").unwrap();
        assert!(!kv_only.permits(&Operation::SecretKeep {
            name: "castle.state".into()
        }));
        assert!(!kv_only.permits(&Operation::EnvRead {
            name: "castle.state".into()
        }));
        let secret_only = GrantSet::parse("secret.keep castle.state\n").unwrap();
        assert!(!secret_only.permits(&Operation::StorageKv {
            scope: "castle.state".into()
        }));
        let fs_only = GrantSet::parse("fs.write /tmp\n").unwrap();
        assert!(!fs_only.permits(&Operation::StorageKv {
            scope: "tmp".into()
        }));
    }

    #[test]
    fn a_spec_parses_into_the_grants_it_names() {
        let set = GrantSet::parse(
            "# a comment\n\nnet.fetch http://127.0.0.1:8080\nfs.read /data\nenv.read NODE_ENV\n",
        )
        .unwrap();
        assert!(set.permits(&Operation::Fetch {
            origin: Origin::new("http", "127.0.0.1", 8080)
        }));
        assert!(set.permits(&Operation::FsRead {
            path: "/data/x".into()
        }));
        assert!(set.permits(&Operation::EnvRead {
            name: "NODE_ENV".into()
        }));
        // And nothing beyond them.
        assert!(!set.permits(&Operation::Fetch {
            origin: Origin::new("http", "127.0.0.1", 9090)
        }));
        assert!(!set.permits(&Operation::FsWrite {
            path: "/data/x".into()
        }));
    }

    #[test]
    fn an_empty_spec_grants_nothing_and_a_bad_one_is_refused() {
        assert!(GrantSet::parse("").unwrap().is_empty());
        assert!(GrantSet::parse("net.fetch").is_err());
        assert!(GrantSet::parse("nonsense.cap x").is_err());
        assert!(GrantSet::parse("fs.read relative/path").is_err());
        assert!(GrantSet::parse("net.fetch not-a-url").is_err());
    }

    #[test]
    fn a_line_with_anything_after_the_target_is_refused_not_truncated() {
        // The typo class this refuses: a split target silently granting its
        // first word.
        assert!(GrantSet::parse("storage.kv payments cache").is_err());
        assert!(GrantSet::parse("secret.keep castle.session old").is_err());
        assert!(GrantSet::parse("fs.read /data /backup").is_err());
        assert!(GrantSet::parse("net.fetch http://a.example # prod").is_err());
        assert!(GrantSet::parse("env.read HOME PATH").is_err());
        // A full-line comment is still a comment.
        assert!(GrantSet::parse("# net.fetch http://a.example extra")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn several_grants_compose_without_widening_each_other() {
        let set = GrantSet::none()
            .with(Grant::Fetch(origin("api.example.com")))
            .with(Grant::FsRead(PathPrefix::new("/data").unwrap()))
            .with(Grant::EnvRead("NODE_ENV".into()));
        assert!(set.permits(&Operation::Fetch {
            origin: origin("api.example.com")
        }));
        assert!(set.permits(&Operation::FsRead {
            path: "/data/x".into()
        }));
        assert!(set.permits(&Operation::EnvRead {
            name: "NODE_ENV".into()
        }));
        assert!(!set.permits(&Operation::FsWrite {
            path: "/data/x".into()
        }));
        assert!(!set.permits(&Operation::Fetch {
            origin: origin("other.example.com")
        }));
        assert!(!set.permits(&Operation::EnvRead {
            name: "AWS_SECRET_ACCESS_KEY".into()
        }));
    }
}
