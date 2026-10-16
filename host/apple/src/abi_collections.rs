//! Collection geometry, logical text and host covers at the native ABI.
use super::*;

impl<D: DataSource> Bridge<D> {
    /// What native containers cover of boxes (LLP 1075.003 §3.5), from the
    /// input buffer's first `len` bytes: LE records of (u32 view, u32 kind —
    /// 0 clears, 1 edges, 2 whole, 3 a scroller's platform inset (top and
    /// bottom) — and f32 top, right, bottom, left), one layout. And the
    /// containing blocks they give subtrees (LLP 1115.002 D8.2): kind 4
    /// with f32 width, height and, for each, 0 for a size, 1 for a maximum,
    /// or 2 for a content-fit limit that an authored size may exceed;
    /// kind 5 takes a block back. Kind 6 says where a block is shown in
    /// place (LLP 1123 D2): f32 x, y, width, height in the local
    /// coordinates of a u32 view after them, which makes its record 28
    /// bytes; kind 7 forgets it.
    pub fn covers(&mut self, len: usize) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let mut records = Vec::new();
        let mut rest = bytes;
        while rest.len() >= 8 {
            let size = if rest[4..8] == 6u32.to_le_bytes() {
                28
            } else {
                24
            };
            let Some((record, after)) = rest.split_at_checked(size) else {
                break;
            };
            records.push(record);
            rest = after;
        }
        let out = if !rest.is_empty() {
            self.refusal("covers: truncated record")
        } else {
            let (mut covers, mut blocks, mut placements) = (Vec::new(), Vec::new(), Vec::new());
            for r in records {
                let word = |i: usize| [r[i], r[i + 1], r[i + 2], r[i + 3]];
                let edge = |i: usize| f32::from_le_bytes(word(i));
                let (view, kind) = (u32::from_le_bytes(word(0)), u32::from_le_bytes(word(4)));
                let axis = |length: usize, policy: usize| match edge(policy) {
                    0.0 => exact_kernel::BlockAxis::Is(edge(length)),
                    2.0 => exact_kernel::BlockAxis::Fit(edge(length)),
                    _ => exact_kernel::BlockAxis::AtMost(edge(length)),
                };
                match kind {
                    4 | 5 => {
                        let block = exact_kernel::HostBlock {
                            width: axis(8, 16),
                            height: axis(12, 20),
                        };
                        blocks.push((view, (kind == 4).then_some(block)));
                        if kind == 5 {
                            placements.push((view, None));
                        }
                    }
                    6 => {
                        let rect = [8, 12, 16, 20].map(|i| f64::from(edge(i)));
                        placements.push((view, Some((rect, u32::from_le_bytes(word(24))))));
                    }
                    7 => placements.push((view, None)),
                    1 => {
                        let edges = [edge(8), edge(12), edge(16), edge(20)];
                        covers.push((view, Some(exact_kernel::HostCover::Edges(edges))));
                    }
                    2 => covers.push((view, Some(exact_kernel::HostCover::Whole))),
                    3 => {
                        let inset = exact_kernel::HostCover::Inset([edge(8), edge(16)]);
                        covers.push((view, Some(inset)));
                    }
                    _ => covers.push((view, None)),
                }
            }
            self.host
                .as_mut()
                .map_or_else(not_booted, |h| h.set_claims(&covers, &blocks, &placements))
        };
        self.emit(out)
    }

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
            .and_then(|h| {
                h.runner()
                    .list_text(view, range, exact_runner::ListTextPolicy::MacOS)
                    .ok()
            })
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

    /// `key\nblock\ninline` in the input buffer (LLP 1070.000 §5); the key may hold a newline.
    pub fn into_view(&mut self, view: u32, len: usize) -> u32 {
        let text = String::from_utf8_lossy(self.input.get(..len).unwrap_or(&[])).into_owned();
        let mut parts = text.rsplitn(3, '\n');
        let (inline, block) = (
            parts.next().unwrap_or("nearest"),
            parts.next().unwrap_or("start"),
        );
        let key = parts.next().unwrap_or("");
        let out = self.host.as_mut().map_or_else(not_booted, |host| {
            host.scroll_into_view(view, key, block, inline)
        });
        self.emit(out)
    }
}
