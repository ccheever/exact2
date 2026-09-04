//! Optional app-supplied delivery composition (LLP 1030 D4).
//! The host owns sessions and assets; the higher adapter owns store policy.
use crate::image::AssetResolver;
use exact_runner::Delivery;
use std::sync::Arc;

/// A pinned plan with its complete asset resolver.
pub struct Selection {
    /// The selected entry, absent for the embedded plan.
    pub entry: Option<String>,
    /// The sequence belonging to this exact entry.
    pub seq: u64,
    /// Validated plan bytes.
    pub plan: Arc<[u8]>,
    /// Complete selected assets; missing names never fall through to embedded files.
    pub assets: AssetResolver,
}

/// Operations the presenter needs from a linked delivery adapter.
/// A binary-only app supplies none and creates no store or check thread.
pub trait Store {
    /// Pin a candidate and its asset resolver for this boot.
    fn prepare_selected(&mut self) -> Option<Selection>;
    /// Count a selected boot.
    fn boot_started(&mut self);
    /// Refuse the selected plan.
    fn entry_refused(&mut self, entry: &str, why: &str);
    /// Refuse assets belonging to the last prepared selection.
    fn selection_corrupt(&mut self, why: &str);
    /// Mark the selected boot successful.
    fn boot_succeeded(&mut self);
    /// Take a boot diagnostic.
    fn take_note(&mut self) -> Option<String>;
    /// The completion wake in the display's poll set.
    fn fd(&self) -> std::os::unix::io::RawFd;
    /// Start a check, returning whether one started.
    fn check(&self) -> bool;
    /// Take the latest asynchronous outcome.
    fn take_line(&mut self) -> Option<String>;
    /// Prepare a complete staged generation without changing live state.
    fn prepare_activation(&self) -> Result<Option<Selection>, String>;
    /// Commit the exact entry and sequence accepted by the host.
    fn commit_activation(&mut self, entry: Option<String>, seq: u64) -> Result<(), String>;
    /// Candidate stream facts, before the live store changes.
    fn staged_stream_into(&self, delivery: &mut Delivery);
    /// Copy the latest delivery facts into the runner.
    fn status_into(&self, delivery: &mut Delivery);
}
