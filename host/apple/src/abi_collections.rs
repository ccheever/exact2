//! Collection geometry and logical text at the native ABI.
use super::*;

impl<D: DataSource> Bridge<D> {
    /// Copy one current immutable region request; stale IDs return an error object.
    pub fn region_request(&mut self, id: u64, known_source: u64) -> u32 {
        let answer = self
            .host
            .as_ref()
            .ok_or_else(|| "not booted".into())
            .and_then(|h| h.region_request_json_known(id, known_source));
        let text = answer.unwrap_or_else(|why| format!("{{\"error\":\"{}\"}}", escape(&why)));
        self.output = text.into_bytes();
        self.output.len() as u32
    }
    /// A worker completed the current paragraph's metrics.
    pub fn text_ready(&mut self, index: u32, generation: u32, revision: u64) -> u32 {
        let text = self
            .host
            .as_mut()
            .map(|h| h.text_ready(exact_kernel::NodeKey { index, generation }, revision))
            .unwrap_or_else(not_booted);
        self.emit(text)
    }
    /// Deliver one native retained artifact; even stale/not-booted takes ownership.
    pub fn region_complete(
        &mut self,
        id: u64,
        metrics: crate::measure::CMetrics,
        owner: Rc<dyn std::any::Any>,
    ) -> u32 {
        let text = self
            .host
            .as_mut()
            .map(|h| {
                h.complete_region_text(
                    id,
                    exact_kernel::TextMetrics {
                        width: metrics.width,
                        height: metrics.height,
                        first_baseline: (metrics.baseline >= 0. || !metrics.baseline.is_finite())
                            .then_some(metrics.baseline),
                    },
                    owner,
                )
            })
            .unwrap_or_else(not_booted);
        self.emit(text)
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

    /// `key\nblock\ninline` in the input buffer (LLP 1070.000 §5).
    pub fn into_view(&mut self, view: u32, len: usize) -> u32 {
        let text = String::from_utf8_lossy(self.input.get(..len).unwrap_or(&[])).into_owned();
        let mut parts = text.split('\n');
        let (key, block, inline) = (
            parts.next().unwrap_or(""),
            parts.next().unwrap_or("start"),
            parts.next().unwrap_or("nearest"),
        );
        let out = self.host.as_mut().map_or_else(not_booted, |host| {
            host.scroll_into_view(view, key, block, inline)
        });
        self.emit(out)
    }
}
