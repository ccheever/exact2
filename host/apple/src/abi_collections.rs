//! Collection geometry and logical text at the native ABI.
use super::*;

impl<D: DataSource> Bridge<D> {
    /// Actual native list scrollport; measured rows are read from kernel layout.
    pub fn list_viewport(&mut self, view: u32, geometry: exact_runner::ListViewport<'_>) -> u32 {
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |h| h.list_viewport(view, geometry));
        self.emit(out)
    }

    /// Resolve an opaque list key, or return the absent-index sentinel.
    pub fn list_index(&self, view: u32, len: usize) -> u32 {
        let key = String::from_utf8_lossy(&self.input[..len.min(self.input.len())]);
        self.host
            .as_ref()
            .and_then(|h| h.runner().list_index(view, &key))
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(u32::MAX)
    }

    /// Logical text for all rows (empty keys), or two UTF-16 endpoints.
    #[allow(clippy::too_many_arguments)]
    pub fn list_text(
        &mut self,
        view: u32,
        first_len: usize,
        len: usize,
        first_paragraph: usize,
        first_offset: usize,
        last_paragraph: usize,
        last_offset: usize,
    ) -> u32 {
        let bytes = &self.input[..len.min(self.input.len())];
        if first_len > bytes.len() {
            return self.emit(String::new());
        }
        let first = String::from_utf8_lossy(&bytes[..first_len]);
        let last = String::from_utf8_lossy(&bytes[first_len..]);
        let range = (first_len != 0).then_some((
            exact_runner::ListTextPosition {
                key: &first,
                paragraph: first_paragraph,
                offset: first_offset,
            },
            exact_runner::ListTextPosition {
                key: &last,
                paragraph: last_paragraph,
                offset: last_offset,
            },
        ));
        let text = self
            .host
            .as_ref()
            .and_then(|h| h.runner().list_text(view, range).ok())
            .unwrap_or_default();
        self.emit(text)
    }

    /// One common LE collection feedback packet in the input buffer. An invalid
    /// length is rejected by decoding an empty packet, never a truncated prefix.
    /// Edge actions can issue requests; publish the batch and submit that work
    /// through the same executor path as an ordinary event.
    pub fn collection_feedback(&mut self, len: usize, now_ms: f64) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let out = self
            .host
            .as_mut()
            .map_or_else(not_booted, |host| host.collection_feedback(bytes, now_ms));
        self.emit(out)
    }
}
