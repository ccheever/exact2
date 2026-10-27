//! Collection geometry, logical text and host covers at the native ABI.
use super::*;

impl<D: DataSource> Bridge<D> {
    /// What native containers cover of boxes (LLP 1075.003 §3.5), from the
    /// input buffer's first `len` bytes: LE records of (u32 view, u32 kind —
    /// 0 clears, 1 edges, 2 whole, 3 a scroller's platform inset (top and
    /// bottom) — and f32 top, right, bottom, left), one layout. And the
    /// containing blocks they give subtrees (LLP 1115.002 D8.2): kind 4
    /// with f32 width, height and, for each, 0 for a size, 1 for a maximum,
    /// 2 for a content-fit limit that an authored size may exceed, or 3 for a
    /// position-area's region (`BlockAxis::Region`);
    /// kind 5 takes a block back. Kinds 6 and 13 place a block (LLP 1123 D2):
    /// f32 x, y, width, height, then the u32 containing view and f32 content
    /// offsets x and y (36 bytes). Kind 6 is a fixed allocation and ignores
    /// the offsets; kind 13 is a scrollport, including at offset zero.
    /// Kind 7 forgets either placement. Kind 8 reports UIKit's grouped-list
    /// card and cell margins: f32 the card's left and right
    /// insets, then the cell's left and right margins; kind 9 clears them.
    /// Kind 12 reports scrollbar gutters in the right and bottom fields.
    /// Kind 14 shows a split's own placeholder (LLP 1116.006 D2,
    /// `Runner::show_split_placeholder`) and kind 15 hides it again; their
    /// four floats are unused.
    pub fn covers(&mut self, len: usize) -> u32 {
        let bytes = self.input.get(..len).unwrap_or(&[]);
        let mut records = Vec::new();
        let mut rest = bytes;
        while rest.len() >= 8 {
            let size = if rest[4..8] == 6u32.to_le_bytes() || rest[4..8] == 13u32.to_le_bytes() {
                36
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
            let (mut covers, mut blocks, mut placements, mut margins, mut lists, mut shows) = (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            for r in records {
                let word = |i: usize| [r[i], r[i + 1], r[i + 2], r[i + 3]];
                let edge = |i: usize| f32::from_le_bytes(word(i));
                let (view, kind) = (u32::from_le_bytes(word(0)), u32::from_le_bytes(word(4)));
                let axis = |length: usize, policy: usize| block_axis(edge(length), edge(policy));
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
                    6 | 13 => {
                        let rect = [8, 12, 16, 20].map(|i| f64::from(edge(i)));
                        let offset = [f64::from(edge(28)), f64::from(edge(32))];
                        if !offset.iter().all(|value| value.is_finite()) {
                            continue;
                        }
                        placements.push((
                            view,
                            Some((
                                rect,
                                u32::from_le_bytes(word(24)),
                                (kind == 13).then_some(offset),
                            )),
                        ));
                    }
                    7 => placements.push((view, None)),
                    8 => margins.push((
                        view,
                        Some(exact_kernel::GroupedMargins {
                            card: [edge(8), edge(12)],
                            cell: [edge(16), edge(20)],
                        }),
                    )),
                    9 => margins.push((view, None)),
                    14 | 15 => shows.push((view, kind == 14)),
                    10 => lists.push((view, Some([edge(8), edge(12)]))),
                    11 => lists.push((view, None)),
                    1 => {
                        let edges = [edge(8), edge(12), edge(16), edge(20)];
                        covers.push((view, Some(exact_kernel::HostCover::Edges(edges))));
                    }
                    2 => covers.push((view, Some(exact_kernel::HostCover::Whole))),
                    12 => covers.push((
                        view,
                        Some(exact_kernel::HostCover::Gutter([edge(12), edge(16)])),
                    )),
                    3 => {
                        let inset = exact_kernel::HostCover::Inset([edge(8), edge(16)]);
                        covers.push((view, Some(inset)));
                    }
                    _ => covers.push((view, None)),
                }
            }
            self.host.as_mut().map_or_else(not_booted, |h| {
                h.set_claims(&covers, &blocks, &placements, &margins, &lists, &shows)
            })
        };
        self.emit(out)
    }

    /// What `view`'s subtree would measure in a block (`Kernel::measure_block`,
    /// a popover's position-area option): the border box's width and height,
    /// then the margins, top, right, bottom and left, as the layout resolved
    /// them, six LE f32s in the output buffer, and 24; 0 when refused. Each axis is a
    /// length and a policy as `covers`' kind 4 takes them, 3 being a region.
    /// No batch: nothing is laid out, published or sent.
    pub fn measure_block(&mut self, view: u32, lengths: [f32; 2], policies: [u32; 2]) -> u32 {
        let block = exact_kernel::HostBlock {
            width: block_axis(lengths[0], policies[0] as f32),
            height: block_axis(lengths[1], policies[1] as f32),
        };
        let used = self
            .host
            .as_mut()
            .and_then(|h| h.measure_block(view, block));
        self.output = used.map_or_else(Vec::new, |m| {
            let [top, right, bottom, left] = m.margin;
            [m.border.0, m.border.1, top, right, bottom, left]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect()
        });
        self.output.len() as u32
    }

    /// The kernel's layout stamp (`Host::layout_stamp`); 0 before a boot.
    pub fn layout_stamp(&self) -> u64 {
        self.host.as_ref().map_or(0, |h| h.layout_stamp())
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

    /// What UIKit covers of list `view`'s port at its start and end, its
    /// bars over a list drawn under them (`Host::set_list_covered`).
    pub fn list_covered(&mut self, view: u32, start: f64, end: f64) {
        if let Some(host) = self.host.as_mut() {
            host.set_list_covered(view, [start, end]);
        }
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

/// One axis of a host block from its wire length and policy: 0 a size, 1 a
/// maximum, 2 a content-fit limit, 3 a position-area's region.
fn block_axis(length: f32, policy: f32) -> exact_kernel::BlockAxis {
    match policy {
        0.0 => exact_kernel::BlockAxis::Is(length),
        2.0 => exact_kernel::BlockAxis::Fit(length),
        3.0 => exact_kernel::BlockAxis::Region(length),
        _ => exact_kernel::BlockAxis::AtMost(length),
    }
}
