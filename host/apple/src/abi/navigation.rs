use super::*;

impl<D: DataSource> Bridge<D> {
    /// `exact_host_back`: the platform's own Back from visit `id` (LLP 1115
    /// D5), a batch.
    pub fn host_back(&mut self, id: u64, now_ms: f64) -> u32 {
        let out = match self.host.as_mut() {
            Some(h) => h.host_back(id, now_ms),
            None => not_booted(),
        };
        self.emit(out)
    }

    /// Follow the UTF-8 location in the owned input buffer.
    pub fn host_go(&mut self, len: usize, now_ms: f64) -> u32 {
        let location = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.host_go(&location, now_ms));
        self.emit(out)
    }
}
