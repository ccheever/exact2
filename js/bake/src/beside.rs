//! What a web build in an app's `dist` keeps beside it, `dist.<name>`: the builds
//! it replaced, those reaped, drives' leases on them, a swap's lock, the tree a
//! swap puts aside, and each build's stage. No source; host/web/beside.mjs holds
//! the same names for the web's scans (host/web/generations.test.mjs compares them).

/// `dist.<name>` for each of these.
pub(crate) const BESIDE: [&str; 5] = ["gen", "gen.reaped", "leases", "lock", "previous"];
/// `dist.<STAGE>…`, a directory each build is made in.
pub(crate) const STAGE: &str = "stage-";

/// Whether `name`, at an app's root, is one a build in `dist` keeps beside it.
pub(crate) fn beside_dist(name: &str) -> bool {
    name.strip_prefix("dist.").is_some_and(|rest| {
        BESIDE.contains(&rest)
            || rest
                .strip_prefix(STAGE)
                .is_some_and(|tail| !tail.is_empty())
    })
}
