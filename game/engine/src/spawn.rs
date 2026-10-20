//! Spawning on world time: a saved schedule and a serial for each kind of thing.
//!
//! A [`Spawner`] is a value, not a resource: a game embeds one per kind of
//! thing in its own resource or component, so it saves, restores and hashes
//! with the game's state. The game calls [`Spawner::due`] from its tick and
//! places each spawn itself, with world randomness (`w.rand`) for anything
//! random. Nothing steps a spawner but that call.

/// When the next spawn is owed, its serial, and the interval, all in ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, crate::Data)]
pub struct Spawner {
    /// The tick the next spawn is owed at.
    pub next: u64,
    /// The serial the next spawn gets; serials count up from zero and never
    /// repeat. At `u32::MAX` the spawner is exhausted and spawns nothing more.
    pub serial: u32,
    /// Ticks between spawns; zero never spawns, which is what `default()` gives.
    pub every: u64,
    /// The most spawns one late call owes. A call that comes after several
    /// intervals have passed owes one per interval up to this; the oldest
    /// beyond it are skipped.
    pub cap: u32,
}

impl Spawner {
    /// A spawner whose first spawn is owed at tick `every` and each next one
    /// `every` ticks later, on the grid of multiples of `every` from tick 0;
    /// a late call owes at most one. Set `next` for another first tick.
    pub fn every(every: u64) -> Self {
        Self {
            next: every,
            serial: 0,
            every,
            cap: 1,
        }
    }
    /// The same spawner owing up to `cap` spawns to a late call.
    pub fn cap(self, cap: u32) -> Self {
        Self { cap, ..self }
    }
    /// The serial of a spawn owed at `tick`, advancing the schedule, or None.
    /// Call it in a loop, `while let Some(serial) = spawner.due(w.tick())`,
    /// so a late call pays everything it owes. A spawner that has given serial
    /// `u32::MAX - 1`, or whose next spawn would fall past tick `u64::MAX`, is
    /// exhausted: it owes nothing more, so no serial repeats.
    pub fn due(&mut self, tick: u64) -> Option<u32> {
        if self.every == 0 || tick < self.next || self.serial == u32::MAX {
            return None;
        }
        let owed = ((tick - self.next) / self.every).saturating_add(1);
        if owed > u64::from(self.cap) {
            // Skip the oldest, keeping the schedule on its grid.
            let skipped = (owed - u64::from(self.cap)).saturating_mul(self.every);
            self.next = self.next.saturating_add(skipped);
            if self.cap == 0 {
                return None;
            }
        }
        let serial = self.serial;
        // A schedule that would pass the end of the clock is exhausted too.
        match self.next.checked_add(self.every) {
            Some(next) => {
                self.serial += 1;
                self.next = next;
            }
            None => self.serial = u32::MAX,
        }
        Some(serial)
    }
}
