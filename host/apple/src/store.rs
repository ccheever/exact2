//! The app's kept secrets on Apple platforms (LLP 1018 D6): `ibex2::host`'s
//! `Secrets` — the Keychain (ibex LLP 0069) — read into a snapshot before the
//! runner boots and written after each commit, on the serial runtime owner,
//! never through Swift.
//!
//! Agent mode (`EXACT_AGENT=1`, LLP 1012) gets a memory store unless
//! `EXACT_STORE=real` says otherwise: a scripted drive starts from nothing
//! and leaves nothing in a developer's keychain.

use ibex2::host::{Bindings, Host};

/// The app's bindings from its grants (LLP 1016 D6): `None` when it declares
/// grants that do not parse — every request is then refused, as before.
pub fn endow(grants: &str) -> Option<Bindings> {
    let set = ibex2::grant::GrantSet::parse(grants).ok()?;
    let mut host = Host::new();
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let real = std::env::var("EXACT_STORE").is_ok_and(|v| v == "real");
    if agent && !real {
        host = host.with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()));
    }
    Some(host.endow(set))
}

/// What the store holds under the granted names — the runner's snapshot.
/// A name the store cannot read is absent, as an ungranted one is.
pub fn snapshot_of(bindings: Option<&Bindings>) -> Vec<(String, String)> {
    let Some(b) = bindings else {
        return Vec::new();
    };
    b.secrets
        .names()
        .iter()
        .filter_map(|n| b.secrets.get(n).ok().flatten().map(|v| (n.to_string(), v)))
        .collect()
}
