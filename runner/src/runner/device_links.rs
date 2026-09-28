//! The device capabilities a host links that are generic over its data
//! source (LLP 1047 D3; LLP 1069's device requests): each capability's
//! entries, or `None` when the artifact doesn't link it, so its code is gone.
//! Native hosts and tests boot with every one ([`DeviceLinks::ALL`], set by
//! the plain boots); the web host passes what its entry registered
//! ([`Runner::set_device_links`]), from what the plan and its data module use
//! ([`crate::uses`]). A plan that uses one unlinked is refused at boot, by
//! name; a request that reaches one unlinked is refused as unlinked.

use super::Runner;
use crate::DataSource;

/// `openAuthSession` (LLP 1069.006): the entries of [`crate::auth`] that
/// check a session, which the core reaches from an agent's answer. Settling
/// and delivering an outcome stay in the core: an unlinked host refuses a
/// session with them.
pub struct AuthLinks<D: DataSource> {
    /// A held session's answer, checked before the hold is spent.
    pub check_answer: fn(&mut Runner<D>, u64, &str) -> Result<(), String>,
}

/// What a runner links of the device capabilities.
pub struct DeviceLinks<D: DataSource> {
    /// `openAuthSession` (LLP 1069.006).
    pub auth: Option<AuthLinks<D>>,
}

impl<D: DataSource> Clone for AuthLinks<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: DataSource> Copy for AuthLinks<D> {}

impl<D: DataSource> Clone for DeviceLinks<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D: DataSource> Copy for DeviceLinks<D> {}

impl<D: DataSource> AuthLinks<D> {
    /// [`crate::auth`]'s entries.
    pub const LINKED: AuthLinks<D> = AuthLinks {
        check_answer: crate::auth::check_answer::<D>,
    };
}

impl<D: DataSource> DeviceLinks<D> {
    /// Every device capability.
    pub const ALL: DeviceLinks<D> = DeviceLinks {
        auth: Some(AuthLinks::LINKED),
    };

    /// The core alone.
    pub const CORE: DeviceLinks<D> = DeviceLinks { auth: None };
}

impl<D: DataSource> Runner<D> {
    /// What this runner links of the device capabilities: the web host's
    /// entry sets it after boot; the plain boots link every one.
    pub fn set_device_links(&mut self, links: DeviceLinks<D>) {
        self.device_links = links;
    }

    /// What this runner links of the device capabilities.
    pub fn device_links(&self) -> DeviceLinks<D> {
        self.device_links
    }
}
