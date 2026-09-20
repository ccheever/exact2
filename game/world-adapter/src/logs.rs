//! Host numeric cursors name bounded snapshots of the kernel's opaque cursor.
use exact_world::{DataError, LogCursor, World};
use std::collections::VecDeque;
pub(crate) struct History {
    cursors: VecDeque<(u64, LogCursor)>,
    next: u64,
}
impl Default for History {
    fn default() -> Self {
        Self {
            cursors: VecDeque::from([(0, LogCursor::default())]),
            next: 0,
        }
    }
}
impl History {
    pub(crate) fn read(&mut self, world: &World, since: u64) -> Result<String, DataError> {
        if since > self.next {
            return Err(DataError::new("future log cursor"));
        }
        let &(from, cursor) = self
            .cursors
            .iter()
            .find(|(id, _)| *id == since)
            .or_else(|| self.cursors.front())
            .ok_or_else(|| DataError::new("missing log cursor"))?;
        let page = world.logs(cursor)?;
        let next = if let Some((id, _)) = self.cursors.iter().find(|(_, c)| *c == page.next) {
            *id
        } else {
            self.next = self
                .next
                .checked_add(1)
                .filter(|n| *n <= 9_007_199_254_740_991)
                .ok_or_else(|| DataError::new("log cursor exhausted"))?;
            if self.cursors.len() == 64 {
                self.cursors.pop_front();
            }
            self.cursors.push_back((self.next, page.next));
            self.next
        };
        Ok(format!("{{\"tick\":{},\"from\":{from},\"next\":{next},\"lines\":{},\"reset\":{},\"truncated\":{}}}",
            world.tick(),page.entries,page.reset,page.truncated || from != since))
    }
}
