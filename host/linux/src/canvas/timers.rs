//! Main-thread observations for Android's timer admission.
use super::*;

impl<D: DataSource + Default> CanvasHost<D> {
    /// Receive the current reader's activity and publication acknowledgement.
    /// Return whether admission made a batch the owner needs to publish.
    pub fn timer_observation(
        &mut self,
        scrolling: bool,
        sampling: bool,
        applying: bool,
        idle: bool,
    ) -> bool {
        let before = self.p.host.runner().kernel().epoch();
        self.p.timer_observation(scrolling, sampling, applying);
        let now = self.now();
        if idle {
            let _ = self.p.admit_idle_timer(now);
        } else {
            let _ = self.p.admit_timer_waits(now, false);
        }
        self.p.host.runner().kernel().epoch() != before
    }

    /// Held kinds, using the same bit positions as the admission ABI.
    pub fn held_timer_waits(&self) -> u32 {
        let waits = self.p.host.runner().waits();
        [
            (waits.frame, 2),
            (waits.update, 4),
            (waits.scrollend, 8),
            (waits.idle, 16),
        ]
        .into_iter()
        .fold(0, |mask, (wait, bit)| {
            mask | if wait.held { bit } else { 0 }
        })
    }
}
