//! Host-backed exclusion for a secret shared by independent module runtimes.

use std::sync::Arc;

/// The persisted secret namespace selected by the host for this session.
pub trait SharedSecrets: Send + Sync {
    /// Start acquiring one name without blocking an executor or module owner.
    /// Wake the host when `take` can return. Before granting a lease, fence
    /// older queued writes so they cannot overwrite its eventual write.
    fn acquire(&self, name: &str, wake: Arc<dyn Fn() + Send + Sync>) -> Box<dyn SecretWait>;
}

/// An acquisition kept outside the host's ordered completion lane.
/// Dropping it cancels an ungranted acquisition and releases an untaken lease.
pub trait SecretWait: Send {
    /// Take the completed acquisition once, or return `None` while waiting.
    fn take(&mut self) -> Option<Result<Arc<dyn SecretLease>, String>>;
}

/// Exclusive access to one persisted secret, held until the last `Arc` drops.
pub trait SecretLease: Send + Sync {
    /// Read the persisted value under the lease. An I/O failure must be an
    /// error; a previous runner snapshot is not a substitute.
    fn read(&self) -> Result<Option<String>, String>;

    /// Persist a value, or remove it, before returning success. A failed
    /// write must never be queued to retry after this lease is released.
    fn write(&self, value: Option<&str>) -> Result<(), String>;

    /// Fence the credential durably before an effectful request escapes the
    /// callback. An abandoned effect must not leave an old rotating token usable.
    fn begin_effect(&self) -> Result<(), String> {
        Ok(())
    }

    /// Finish a successful callback after its writes are durable. A cancelled
    /// or failed callback drops its lease without calling this method.
    fn finish(&self) -> Result<(), String> {
        Ok(())
    }
}
