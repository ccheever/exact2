//! Which source code units are hidden, where a caret may sit, and what a line
//! draws in place of its prefix. Hidden syntax stays in the source; a host
//! keeps it out of layout and out of the accessibility tree.

use exact_markdown::{ParagraphKind as K, Range, Replacement, Styled, MARKER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RunKind {
    /// An inline opener or closer.
    Inline,
    /// A line prefix: heading hashes, list markers, quote markers, fences.
    Prefix,
    /// A link's `](target)`.
    LinkClose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    pub start: u32,
    pub end: u32,
    pub kind: RunKind,
}

/// What a paragraph's first line draws in place of its hidden prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deco {
    /// A bullet.
    Bullet,
    /// The item's number, drawn from this source range.
    Number(u32, u32),
    /// A checkbox, checked or not.
    Task(bool),
    /// A thematic break.
    Rule,
}

#[derive(Debug, Default)]
pub(crate) struct Projection {
    pub runs: Vec<Run>,
    /// Adjacent runs merged: what a caret moves across as one step.
    pub clusters: Vec<(u32, u32)>,
    /// By paragraph start, in order.
    pub decorations: Vec<(u32, Deco)>,
    /// Fence lines, their newline included.
    pub collapsed: Vec<(u32, u32)>,
    prefix_sum: Vec<u32>,
}

impl Projection {
    pub fn new(source: &[u16], styled: &Styled) -> Self {
        let len = source.len() as u32;
        let ch = |i: u32| if i < len { source[i as usize] } else { 0 };
        // The styler's lists are sorted by start; where two share one, the
        // last is the one that counts.
        fn last<T>(list: &[T], at: u32, start: impl Fn(&T) -> u32) -> Option<&T> {
            let k = list.partition_point(|x| start(x) <= at);
            list[..k].last().filter(|x| start(x) == at)
        }
        let hidden = |at| last(&styled.hidden, at, |h| h.start).map(|h| h.end);
        let replaced = |at| last(&styled.replaced, at, |r| r.range.start);
        let marker = |at| {
            last(&styled.spans, at, |s| s.range.start)
                .filter(|s| s.style & MARKER != 0)
                .map(|s| s.range.end)
        };
        let mut out = Self::default();
        let mut prefixed = Vec::new();
        let mut prefixes = Vec::new();
        for p in &styled.paragraphs {
            let Range { start, end } = p.range;
            if p.kind == K::Fence {
                let end = if ch(end) == 10 { end + 1 } else { end };
                prefixes.push(Run {
                    start,
                    end,
                    kind: RunKind::Prefix,
                });
                out.collapsed.push((start, end));
                prefixed.push(start);
                continue;
            }
            let listy = matches!(p.kind, K::Bullet | K::Ordered | K::Task(_));
            if !(listy || p.quote > 0 || matches!(p.kind, K::Heading(_) | K::Rule)) {
                continue;
            }
            let mut at = start;
            let mut deco = None;
            while at < end {
                if let Some(h) = hidden(at) {
                    prefixed.push(at);
                    if h <= at {
                        break;
                    }
                    at = h;
                } else if let Some(r) = replaced(at) {
                    deco = Some(match &r.with {
                        Replacement::Footnote(_) => break,
                        Replacement::Bullet => Deco::Bullet,
                        Replacement::TaskBox(done) => Deco::Task(*done),
                        Replacement::Rule => Deco::Rule,
                    });
                    let r = r.range.end;
                    if r <= at {
                        break;
                    }
                    at = r + u32::from(ch(r) == 32);
                } else if let (Some(m), K::Ordered) = (marker(at), &p.kind) {
                    deco = Some(Deco::Number(at, m));
                    if m <= at {
                        break;
                    }
                    at = m + u32::from(ch(m) == 32);
                } else if (listy || p.quote > 0) && matches!(ch(at), 32 | 9) {
                    at += 1;
                } else {
                    break;
                }
            }
            if at > start {
                prefixes.push(Run {
                    start,
                    end: at,
                    kind: RunKind::Prefix,
                });
            }
            if let Some(d) = deco {
                out.decorations.push((start, d));
            }
        }
        // Prefixes and the remaining inline syntax, each already in order,
        // merged by start (prefixes first on a tie).
        let inline = styled
            .hidden
            .iter()
            .filter(|h| prefixed.binary_search(&h.start).is_err())
            .map(|h| {
                let link = ch(h.start) == u16::from(b']') && ch(h.start + 1) == u16::from(b'(');
                Run {
                    start: h.start,
                    end: h.end,
                    kind: if link {
                        RunKind::LinkClose
                    } else {
                        RunKind::Inline
                    },
                }
            });
        let (mut prefixes, mut inline) = (prefixes.into_iter().peekable(), inline.peekable());
        while let Some(r) = match (prefixes.peek(), inline.peek()) {
            (Some(p), Some(i)) if i.start < p.start => inline.next(),
            (Some(_), _) => prefixes.next(),
            _ => inline.next(),
        } {
            if let Some(last) = out.runs.last_mut() {
                if r.start < last.end {
                    last.end = last.end.max(r.end);
                    continue;
                }
            }
            if r.end > r.start {
                out.runs.push(r);
            }
        }
        let mut before = 0;
        for r in &out.runs {
            match out.clusters.last_mut() {
                Some(last) if last.1 == r.start => last.1 = r.end,
                _ => {
                    out.prefix_sum.push(before);
                    out.clusters.push((r.start, r.end));
                }
            }
            before += r.end - r.start;
        }
        out
    }

    /// The cluster with `start <= p <= end`.
    fn cluster_index(&self, p: u32) -> Option<usize> {
        let k = self.clusters.partition_point(|c| c.1 < p);
        self.clusters.get(k).filter(|c| c.0 <= p).map(|_| k)
    }

    pub fn is_hidden(&self, i: u32) -> bool {
        self.cluster_index(i)
            .is_some_and(|k| i >= self.clusters[k].0 && i < self.clusters[k].1)
    }

    /// The hidden stretch around `p`, or the empty range at `p`.
    pub fn cluster(&self, p: u32) -> (u32, u32) {
        self.cluster_index(p).map_or((p, p), |k| self.clusters[k])
    }

    fn runs_in(&self, (s, e): (u32, u32)) -> impl Iterator<Item = &Run> {
        let from = self.runs.partition_point(|r| r.start < s);
        self.runs[from..].iter().take_while(move |r| r.end <= e)
    }

    /// `p` counted in visible code units.
    pub fn visible(&self, p: u32) -> u32 {
        let k = self.clusters.partition_point(|c| c.0 < p);
        if k == 0 {
            return p;
        }
        let c = self.clusters[k - 1];
        p - self.prefix_sum[k - 1] - (c.1 - c.0).min(p - c.0)
    }

    /// Where a caret at `p` sits: before inline openers and closers, so it
    /// takes the preceding style; after line prefixes and link closers.
    pub fn canonical(&self, p: u32) -> u32 {
        let Some(k) = self.cluster_index(p) else {
            return p;
        };
        let c = self.clusters[k];
        let mut at = c.0;
        for r in self.runs_in(c) {
            if r.start != at {
                continue;
            }
            if matches!(r.kind, RunKind::Prefix | RunKind::LinkClose) {
                at = r.end;
            } else {
                break;
            }
        }
        at
    }

    /// The parts of `s..e` no hidden run covers.
    pub fn visible_parts(&self, s: u32, e: u32) -> Vec<(u32, u32)> {
        let mut parts = Vec::new();
        let mut at = s;
        for &(cs, ce) in &self.clusters {
            if ce <= s {
                continue;
            }
            if cs >= e {
                break;
            }
            if cs > at {
                parts.push((at, cs));
            }
            at = at.max(ce);
        }
        if at < e {
            parts.push((at, e));
        }
        parts
    }

    pub fn prefix_run(&self, s: u32, e: u32) -> Option<Run> {
        self.runs
            .iter()
            .find(|r| r.kind == RunKind::Prefix && r.start <= s && e <= r.end)
            .copied()
    }

    pub fn hidden_indices(&self) -> impl Iterator<Item = u32> + '_ {
        self.runs.iter().flat_map(|r| r.start..r.end)
    }

    pub fn visible_text(&self, source: &[u16]) -> Vec<u16> {
        let mut out = Vec::with_capacity(source.len());
        let mut at = 0;
        for &(s, e) in &self.clusters {
            out.extend_from_slice(&source[at..s as usize]);
            at = e as usize;
        }
        out.extend_from_slice(&source[at.min(source.len())..]);
        out
    }
}
