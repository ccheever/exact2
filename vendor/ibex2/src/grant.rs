//! The shared grant grammar, with filesystem realization owned by this executor.
//! Exact patch: LLP 1016 D6 / LLP 1018 D3.
pub use exact_grants::*;

pub fn realized_fs(grants: &GrantSet) -> GrantSet {
    grants.map_fs(|path| {
        crate::stdlib::fs::realize(std::path::Path::new(path))
            .to_string_lossy()
            .into_owned()
    })
}
