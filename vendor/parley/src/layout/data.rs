// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::{ContentWidths, Glyph, LineMetrics, RunMetrics, Style};
use crate::style::Brush;
use crate::util::nearly_zero;
use crate::{FontData, IndentOptions, InlineBoxKind, LineHeight, OverflowWrap, TextWrapMode};
use core::ops::Range;

use alloc::vec::Vec;

use crate::analysis::cluster::Whitespace;
use crate::analysis::{Boundary, CharInfo};

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ClusterData {
    pub(crate) info: ClusterInfo,
    /// Cluster flags (see impl methods for details).
    pub(crate) flags: u16,
    /// Style index for this cluster.
    pub(crate) style_index: u16,
    /// Number of glyphs in this cluster (0xFF = single glyph stored inline)
    pub(crate) glyph_len: u8,
    /// Number of text bytes in this cluster
    pub(crate) text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub(crate) glyph_offset: u32,
    /// Offset into the text for this cluster
    pub(crate) text_offset: u16,
    /// Advance width for this cluster
    pub(crate) advance: f32,
}

impl ClusterData {
    pub(crate) const LIGATURE_START: u16 = 1;
    pub(crate) const LIGATURE_COMPONENT: u16 = 2;

    #[inline(always)]
    pub(crate) fn is_ligature_start(self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    #[inline(always)]
    pub(crate) fn is_ligature_component(self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }

    #[inline(always)]
    pub(crate) fn text_range(self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ClusterInfo {
    boundary: Boundary,
    source_char: char,
}

impl ClusterInfo {
    pub(crate) fn new(boundary: Boundary, source_char: char) -> Self {
        Self {
            boundary,
            source_char,
        }
    }

    // Returns the boundary type of the cluster.
    pub(crate) fn boundary(self) -> Boundary {
        self.boundary
    }

    // Returns the whitespace type of the cluster.
    pub(crate) fn whitespace(self) -> Whitespace {
        to_whitespace(self.source_char)
    }

    /// Returns if the cluster is a line boundary.
    pub(crate) fn is_boundary(self) -> bool {
        self.boundary != Boundary::None
    }

    /// Returns if the cluster is an emoji.
    pub(crate) fn is_emoji(self) -> bool {
        // TODO: Defer to ICU4X properties (see: https://docs.rs/icu/latest/icu/properties/props/struct.Emoji.html).
        matches!(self.source_char as u32, 0x1F600..=0x1F64F | 0x1F300..=0x1F5FF | 0x1F680..=0x1F6FF | 0x2600..=0x26FF | 0x2700..=0x27BF)
    }

    /// Returns if the cluster is any whitespace.
    pub(crate) fn is_whitespace(self) -> bool {
        self.source_char.is_whitespace()
    }

    /// Returns the cluster's original character.
    pub(crate) fn source_char(self) -> char {
        self.source_char
    }
}

const fn to_whitespace(c: char) -> Whitespace {
    const LINE_SEPARATOR: char = '\u{2028}';
    const PARAGRAPH_SEPARATOR: char = '\u{2029}';

    match c {
        ' ' => Whitespace::Space,
        '\t' => Whitespace::Tab,
        '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => Whitespace::Newline,
        '\u{00A0}' => Whitespace::NoBreakSpace,
        _ => Whitespace::None,
    }
}

/// `HarfRust`-based run data
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunData {
    /// Index of the font for the run.
    pub(crate) font_index: usize,
    /// Font size.
    pub(crate) font_size: f32,
    /// Font attributes, needed for accessibility.
    pub(crate) font_attrs: fontique::Attributes,
    /// Synthesis for rendering (contains variation settings)
    pub(crate) synthesis: fontique::Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub(crate) coords_range: Range<usize>,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Bidi level for the run.
    pub(crate) bidi_level: u8,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub(crate) glyph_start: usize,
    /// Metrics for the run.
    pub(crate) metrics: RunMetrics,
    /// Additional word spacing.
    pub(crate) word_spacing: f32,
    /// Additional letter spacing.
    pub(crate) letter_spacing: f32,
    /// Total advance of the run.
    pub(crate) advance: f32,
}

#[derive(Copy, Clone, Default, PartialEq, Debug)]
pub enum BreakReason {
    #[default]
    None,
    Regular,
    Explicit,
    Emergency,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub(crate) struct LineData {
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of line items.
    pub(crate) item_range: Range<usize>,
    /// Metrics for the line.
    pub(crate) metrics: LineMetrics,
    /// The cause of the line break.
    pub(crate) break_reason: BreakReason,
    /// Maximum advance for the line.
    pub(crate) max_advance: f32,
    /// Number of justified clusters on the line.
    pub(crate) num_spaces: usize,
    /// Text indent applied to this line.
    pub(crate) indent: f32,
}

impl LineData {
    pub(crate) fn size(&self) -> f32 {
        self.metrics.ascent + self.metrics.descent + self.metrics.leading
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LineItemData {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
    /// Advance (size in direction of text flow) for the run.
    pub(crate) advance: f32,

    // Fields that only apply to text runs (Ignored for boxes)
    // TODO: factor this out?
    /// True if the run is composed entirely of whitespace.
    pub(crate) is_whitespace: bool,
    /// True if the run ends in whitespace.
    pub(crate) has_trailing_whitespace: bool,
    /// Range of the source text.
    pub(crate) text_range: Range<usize>,
    /// Range of clusters.
    pub(crate) cluster_range: Range<usize>,
}

impl LineItemData {
    pub(crate) fn is_text_run(&self) -> bool {
        self.kind == LayoutItemKind::TextRun
    }

    /// If the item is a text run
    ///   - Determine if it consists entirely of whitespace (`is_whitespace` property)
    ///   - Determine if it has trailing whitespace (`has_trailing_whitespace` property)
    pub(crate) fn compute_whitespace_properties<B: Brush>(&mut self, layout_data: &LayoutData<B>) {
        // Skip items which are not text runs
        if self.kind != LayoutItemKind::TextRun {
            return;
        }

        self.is_whitespace = true;
        // Clusters are in logical order in RTL runs too (they are reversed
        // after shaping), so trailing whitespace is at the end in both.
        for cluster in layout_data.clusters[self.cluster_range.clone()]
            .iter()
            .rev()
        {
            if cluster.info.is_whitespace() {
                self.has_trailing_whitespace = true;
            } else {
                self.is_whitespace = false;
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub(crate) kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or `inline_boxes` vec
    pub(crate) index: usize,
    /// Bidi level for the item (used for reordering)
    pub(crate) bidi_level: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutData<B: Brush> {
    // General settings (directly from the "builder")
    /// The display scale factor
    pub(crate) scale: f32,
    /// Whether metrics should be quantized to pixel boundaries
    pub(crate) quantize: bool,
    /// The `BiDi` base level
    pub(crate) base_level: u8,
    /// The line box's level: the base level unless the builder set a line
    /// direction (trailing whitespace and alignment follow it).
    pub(crate) line_level: u8,
    /// The length of the text in the layout
    pub(crate) text_len: usize,

    // Output of style resolution (input to line breaking)
    pub(crate) styles: Vec<Style<B>>,
    pub(crate) inline_boxes: Vec<InlineBox>,

    // Output of shaping (input to line breaking)
    pub(crate) fonts: Vec<FontData>,
    pub(crate) coords: Vec<i16>,
    pub(crate) runs: Vec<RunData>,
    pub(crate) items: Vec<LayoutItem>,
    pub(crate) clusters: Vec<ClusterData>,
    pub(crate) glyphs: Vec<Glyph>,

    // Output of line breaking
    /// The lines in the
    pub(crate) lines: Vec<LineData>,
    /// Items within each line
    pub(crate) line_items: Vec<LineItemData>,
    /// The width constraint that was used to line break the layout
    pub(crate) layout_max_advance: f32,
    /// The computed width of the layout excluding trailing whitespace
    pub(crate) width: f32,
    /// The computed width of the layout including trailing whitespace
    pub(crate) full_width: f32,
    /// The computed height of the layout
    pub(crate) height: f32,

    // Output of alignment
    #[cfg(feature = "accesskit")]
    /// Directly store the alignment if accessibility is enabled so we can
    /// set the corresponding AccessKit property.
    pub(crate) alignment: Option<super::Alignment>,
    /// Whether the layout is aligned with [`crate::Alignment::Justify`].
    pub(crate) is_aligned_justified: bool,
    /// The text-indent amount in layout units.
    pub(crate) indent_amount: f32,
    /// Options controlling text-indent behavior (each-line, hanging).
    pub(crate) indent_options: IndentOptions,

    /// Each soft hyphen's (U+00AD) cluster, with the hyphen its run's face
    /// shows when a line breaks there: glyph id and advance.
    pub(crate) soft_hyphens: Vec<(usize, u32, f32)>,
    /// The soft-hyphen clusters a line break turned into hyphens, and the
    /// tabs it set to their stops, as they were, so the next line breaking
    /// restores them.
    pub(crate) hyphenated: Vec<(usize, ClusterData)>,
    /// Each tab's cluster, its run's space glyph and its tab stop interval
    /// (CSS `tab-size: 8`: eight spaces, spacing included).
    pub(crate) tabs: Vec<(usize, u32, f32)>,
    /// The spaces justification widened, with their own advances.
    pub(crate) justified: Vec<(usize, f32)>,
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            quantize: true,
            base_level: 0,
            line_level: 0,
            text_len: 0,
            width: 0.,
            full_width: 0.,
            height: 0.,
            fonts: Vec::new(),
            coords: Vec::new(),
            styles: Vec::new(),
            inline_boxes: Vec::new(),
            runs: Vec::new(),
            items: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            line_items: Vec::new(),
            #[cfg(feature = "accesskit")]
            alignment: None,
            is_aligned_justified: false,
            layout_max_advance: 0.0,
            indent_amount: 0.0,
            indent_options: IndentOptions::default(),
            soft_hyphens: Vec::new(),
            hyphenated: Vec::new(),
            tabs: Vec::new(),
            justified: Vec::new(),
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub(crate) fn clear(&mut self) {
        self.scale = 1.;
        self.quantize = true;
        self.base_level = 0;
        self.line_level = 0;
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.height = 0.;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.runs.clear();
        self.items.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.line_items.clear();
        self.soft_hyphens.clear();
        self.hyphenated.clear();
        self.tabs.clear();
        self.justified.clear();
    }

    /// A tab's space glyph and stop interval.
    pub(crate) fn tab(&self, cluster: usize) -> Option<(u32, f32)> {
        let i = self.tabs.binary_search_by_key(&cluster, |t| t.0).ok()?;
        let (_, glyph, interval) = self.tabs[i];
        Some((glyph, interval))
    }

    /// Set a tab to `advance` (to its next stop) until the next line
    /// breaking; it draws as its run's space.
    pub(crate) fn set_tab(&mut self, cluster: usize, advance: f32) {
        let Some((glyph, _)) = self.tab(cluster) else {
            return;
        };
        let data = &mut self.clusters[cluster];
        if data.glyph_len != 0xFF {
            return;
        }
        self.hyphenated.push((cluster, *data));
        data.glyph_offset = glyph;
        data.advance = advance;
    }

    /// The hyphen a soft hyphen at `cluster` shows when a line breaks
    /// after it: glyph id and advance.
    pub(crate) fn soft_hyphen(&self, cluster: usize) -> Option<(u32, f32)> {
        let i = self
            .soft_hyphens
            .binary_search_by_key(&cluster, |h| h.0)
            .ok()?;
        let (_, glyph, advance) = self.soft_hyphens[i];
        Some((glyph, advance))
    }

    /// Show the hyphen at a soft hyphen a line ends with: its cluster takes
    /// the hyphen's glyph and advance until the next line breaking.
    pub(crate) fn hyphenate(&mut self, cluster: usize) -> f32 {
        let Some((glyph, advance)) = self.soft_hyphen(cluster) else {
            return 0.0;
        };
        let data = &mut self.clusters[cluster];
        if data.glyph_len != 0xFF {
            return 0.0;
        }
        self.hyphenated.push((cluster, *data));
        data.glyph_offset = glyph;
        data.advance = advance;
        advance
    }

    /// Undo every hyphen the last line breaking showed.
    pub(crate) fn unhyphenate(&mut self) {
        for (cluster, data) in self.hyphenated.drain(..).rev() {
            self.clusters[cluster] = data;
        }
    }

    /// Push an inline box to the list of items
    pub(crate) fn push_inline_box(&mut self, index: usize) {
        // Give the box the same bidi level as the preceding text run
        // (or else default to 0 if there is not yet a text run)
        let bidi_level = self.runs.last().map(|r| r.bidi_level).unwrap_or(0);

        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
            bidi_level,
        });
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn push_run(
        &mut self,
        font: FontData,
        font_size: f32,
        font_attrs: fontique::Attributes,
        synthesis: fontique::Synthesis,
        glyph_buffer: &harfrust::GlyphBuffer,
        bidi_level: u8,
        style_index: u16,
        word_spacing: f32,
        letter_spacing: f32,
        source_text: &str,
        char_infos: &[(CharInfo, u16)], // From text analysis
        text_range: Range<usize>,       // The text range this run covers
        coords: &[harfrust::NormalizedCoord],
    ) {
        let coords_start = self.coords.len();
        self.coords.extend(coords.iter().map(|c| c.to_bits()));
        let coords_end = self.coords.len();

        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });

        let metrics = {
            let font = &self.fonts[font_index];
            let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), font.index).unwrap();
            skrifa::metrics::Metrics::new(&font_ref, skrifa::prelude::Size::new(font_size), coords)
        };
        let units_per_em = metrics.units_per_em as f32;

        let metrics = {
            let (underline_offset, underline_size) = if let Some(underline) = metrics.underline {
                (underline.offset, underline.thickness)
            } else {
                // Default values from Harfbuzz: https://github.com/harfbuzz/harfbuzz/blob/00492ec7df0038f41f78d43d477c183e4e4c506e/src/hb-ot-metrics.cc#L334
                let default = units_per_em / 18.0;
                (default, default)
            };
            let (strikethrough_offset, strikethrough_size) =
                if let Some(strikeout) = metrics.strikeout {
                    (strikeout.offset, strikeout.thickness)
                } else {
                    // Default values from HarfBuzz: https://github.com/harfbuzz/harfbuzz/blob/00492ec7df0038f41f78d43d477c183e4e4c506e/src/hb-ot-metrics.cc#L334-L347
                    (metrics.ascent / 2.0, units_per_em / 18.0)
                };

            // Compute line height
            let style = &self.styles[style_index as usize];
            let line_height = match style.line_height {
                LineHeight::Absolute(value) => value,
                LineHeight::FontSizeRelative(value) => value * font_size,
                LineHeight::MetricsRelative(value) => {
                    (metrics.ascent - metrics.descent + metrics.leading) * value
                }
            };

            RunMetrics {
                ascent: metrics.ascent,
                descent: -metrics.descent,
                leading: metrics.leading,
                underline_offset,
                underline_size,
                strikethrough_offset,
                strikethrough_size,
                line_height,
                x_height: metrics.x_height,
                cap_height: metrics.cap_height,
            }
        };

        let cluster_range = self.clusters.len()..self.clusters.len();

        let mut run = RunData {
            font_index,
            font_size,
            font_attrs,
            synthesis,
            coords_range: coords_start..coords_end,
            text_range,
            bidi_level,
            cluster_range,
            glyph_start: self.glyphs.len(),
            metrics,
            word_spacing,
            letter_spacing,
            advance: 0.,
        };

        // `HarfRust` returns glyphs in visual order, so we need to process them as such while
        // maintaining logical ordering of clusters.

        let glyph_infos = glyph_buffer.glyph_infos();
        if glyph_infos.is_empty() {
            return;
        }
        let glyph_positions = glyph_buffer.glyph_positions();
        let scale_factor = font_size / units_per_em;
        let cluster_range_start = self.clusters.len();
        let is_rtl = bidi_level & 1 == 1;
        if !is_rtl {
            run.advance = process_clusters(
                Direction::Ltr,
                &mut self.clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos,
                glyph_positions,
                char_infos,
                source_text.char_indices(),
            );
        } else {
            run.advance = process_clusters(
                Direction::Rtl,
                &mut self.clusters,
                &mut self.glyphs,
                scale_factor,
                glyph_infos,
                glyph_positions,
                char_infos,
                source_text.char_indices().rev(),
            );
            // Reverse clusters into logical order for RTL
            let clusters_len = self.clusters.len();
            self.clusters[cluster_range_start..clusters_len].reverse();
        }

        run.cluster_range = cluster_range_start..self.clusters.len();
        if source_text.contains('\t') {
            let font = &self.fonts[font_index];
            let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), font.index).unwrap();
            let space = {
                use skrifa::MetadataProvider;
                font_ref.charmap().map(' ').map(|glyph| {
                    let metrics = font_ref
                        .glyph_metrics(skrifa::prelude::Size::new(font_size), coords);
                    (glyph.to_u32(), metrics.advance_width(glyph).unwrap_or(0.0))
                })
            };
            if let Some((glyph, advance)) = space {
                let interval = 8.0 * (advance + letter_spacing + word_spacing);
                if interval > 0.0 {
                    for index in run.cluster_range.clone() {
                        if self.clusters[index].info.source_char() == '\t' {
                            self.tabs.push((index, glyph, interval));
                        }
                    }
                }
            }
        }
        if source_text.contains('\u{ad}') {
            let font = &self.fonts[font_index];
            let font_ref = skrifa::FontRef::from_index(font.data.as_ref(), font.index).unwrap();
            let hyphen = {
                use skrifa::MetadataProvider;
                font_ref.charmap().map('-').map(|glyph| {
                    let metrics = font_ref
                        .glyph_metrics(skrifa::prelude::Size::new(font_size), coords);
                    (glyph.to_u32(), metrics.advance_width(glyph).unwrap_or(0.0))
                })
            };
            if let Some((glyph, advance)) = hyphen {
                for index in run.cluster_range.clone() {
                    if self.clusters[index].info.source_char() == '\u{ad}' {
                        self.soft_hyphens.push((index, glyph, advance));
                    }
                }
            }
        }
        if !run.cluster_range.is_empty() {
            self.runs.push(run);
            self.items.push(LayoutItem {
                kind: LayoutItemKind::TextRun,
                index: self.runs.len() - 1,
                bidi_level,
            });
        }
    }

    pub(crate) fn finish(&mut self) {
        for run in &self.runs {
            let word = run.word_spacing;
            let letter = run.letter_spacing;
            if nearly_zero(word) && nearly_zero(letter) {
                continue;
            }
            let clusters = &mut self.clusters[run.cluster_range.clone()];
            for cluster in clusters {
                let mut spacing = letter;
                if !nearly_zero(word) && cluster.info.whitespace().is_space_or_nbsp() {
                    spacing += word;
                }
                if !nearly_zero(spacing) {
                    cluster.advance += spacing;
                    if cluster.glyph_len != 0xFF {
                        let start = run.glyph_start + cluster.glyph_offset as usize;
                        let end = start + cluster.glyph_len as usize;
                        let glyphs = &mut self.glyphs[start..end];
                        if let Some(last) = glyphs.last_mut() {
                            last.advance += spacing;
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn calculate_content_widths(&self) -> ContentWidths {
        // Clusters are stored in logical order in every run (RTL runs are
        // reversed after shaping), so the whitespace that hangs at a break is
        // the run of whitespace clusters logically before it, in either
        // direction. All of it hangs, not only the last cluster.
        //
        // `text-indent` is part of an indented line's width: a min-content
        // segment or max-content line that starts an indented line includes
        // it, as `BreakLines` does (`resolve_indent`).
        let indent = self.indent_amount;
        let opts = self.indent_options;
        let indent_for = |scope_start: bool| {
            if scope_start ^ opts.hanging {
                indent
            } else {
                0.0
            }
        };

        let mut min_width = 0.0_f32;
        let mut max_width = 0.0_f32;

        let mut running_min_width = indent_for(true);
        let mut running_max_width = indent_for(true);
        let mut trailing_whitespace = 0.0_f32;
        let mut text_wrap_mode = TextWrapMode::Wrap;
        for item in &self.items {
            match item.kind {
                LayoutItemKind::TextRun => {
                    let run = &self.runs[item.index];
                    let clusters = &self.clusters[run.cluster_range.clone()];
                    for (offset, cluster) in clusters.iter().enumerate() {
                        let index = run.cluster_range.start + offset;
                        let boundary = cluster.info.boundary();
                        let style = &self.styles[cluster.style_index as usize];
                        let prev_text_wrap_mode = text_wrap_mode;
                        text_wrap_mode = style.text_wrap_mode;
                        if boundary == Boundary::Mandatory
                            || (prev_text_wrap_mode == TextWrapMode::Wrap
                                && (boundary == Boundary::Line
                                    || style.overflow_wrap == OverflowWrap::Anywhere))
                        {
                            // A segment that ends at a soft hyphen shows one.
                            let hyphen = index
                                .checked_sub(1)
                                .filter(|_| boundary == Boundary::Line)
                                .and_then(|i| self.soft_hyphen(i))
                                .map_or(0.0, |h| h.1);
                            min_width = min_width
                                .max(running_min_width - trailing_whitespace + hyphen);
                            if boundary == Boundary::Mandatory {
                                max_width = max_width.max(running_max_width - trailing_whitespace);
                                running_max_width = indent_for(opts.each_line);
                                running_min_width = indent_for(opts.each_line);
                            } else {
                                running_min_width = indent_for(false);
                            }
                        }
                        // A soft hyphen is invisible unless a line ends at
                        // it (and then shows the advance set above).
                        let advance = if cluster.info.source_char() == '\u{ad}' {
                            0.0
                        } else {
                            cluster.advance
                        };
                        running_min_width += advance;
                        running_max_width += advance;
                        if cluster.info.whitespace().is_space_or_nbsp() {
                            trailing_whitespace += advance;
                        } else {
                            trailing_whitespace = 0.0;
                        }
                    }
                    min_width = min_width.max(running_min_width - trailing_whitespace);
                }
                LayoutItemKind::InlineBox => {
                    let ibox = &self.inline_boxes[item.index];
                    if ibox.kind == InlineBoxKind::InFlow {
                        running_max_width += ibox.width;
                        if text_wrap_mode == TextWrapMode::Wrap {
                            min_width = min_width.max(running_min_width - trailing_whitespace);
                            min_width = min_width.max(ibox.width);
                            running_min_width = 0.0;
                        } else {
                            running_min_width += ibox.width;
                        }
                    }
                    trailing_whitespace = 0.0;
                }
            }
            max_width = max_width.max(running_max_width - trailing_whitespace);
        }

        min_width = min_width.max(running_min_width - trailing_whitespace);

        ContentWidths {
            min: min_width,
            max: max_width,
        }
    }
}

/// Processes shaped glyphs from `HarfRust` and converts them into `ClusterData` and `Glyph`.
///
/// # Parameters
///
/// ## Output Parameters (mutated by this function):
/// * `clusters` - Vector where new `ClusterData` entries will be pushed.
/// * `glyphs` - Vector where new `Glyph` entries will be pushed. Note: single-glyph clusters
///   with zero offsets may be inlined directly into `ClusterData`.
///
/// ## Input Parameters:
/// * `direction` - Direction of the text.
/// * `scale_factor` - Scaling factor used to convert font units to the target size.
/// * `glyph_infos` - `HarfRust` glyph information in visual order.
/// * `glyph_positions` - `HarfRust` glyph positioning data in visual order.
/// * `char_infos` - Character information from text analysis, indexed by cluster ID.
/// * `char_indices_iter` - Iterator over (`byte_offset`, `char`) pairs from the source text.
///   Should be in logical order (forward for LTR, reverse for RTL).
fn process_clusters<I: Iterator<Item = (usize, char)>>(
    direction: Direction,
    clusters: &mut Vec<ClusterData>,
    glyphs: &mut Vec<Glyph>,
    scale_factor: f32,
    glyph_infos: &[harfrust::GlyphInfo],
    glyph_positions: &[harfrust::GlyphPosition],
    char_infos: &[(CharInfo, u16)],
    char_indices_iter: I,
) -> f32 {
    let mut char_indices_iter = char_indices_iter.peekable();
    let mut cluster_start_char = char_indices_iter.next().unwrap();
    let mut total_glyphs: u32 = 0;
    let mut cluster_glyph_offset: u32 = 0;
    let start_cluster_id = glyph_infos.first().unwrap().cluster;
    let mut cluster_id = start_cluster_id;
    let mut char_info = char_infos[cluster_id as usize];
    let mut run_advance = 0.0;
    let mut cluster_advance = 0.0;
    // If the current cluster might be a single-glyph, zero-offset cluster, we defer
    // pushing the first glyph to `glyphs` because it might be inlined into `ClusterData`.
    let mut pending_inline_glyph: Option<Glyph> = None;

    // The mental model for understanding this function is best grasped by first reading
    // the HarfBuzz docs on [clusters](https://harfbuzz.github.io/working-with-harfbuzz-clusters.html).
    //
    // `num_components` is the number of characters in the current cluster. Since source text's characters
    // were inserted into `HarfRust`'s buffer using their logical indices as the cluster ID, `HarfRust` will
    // assign the first character's cluster ID (in logical order) to the merged cluster because the minimum
    // ID is selected for [merging](https://github.com/harfbuzz/harfrust/blob/a38025fb336230b492366740c86021bb406bcd0d/src/hb/buffer.rs#L920-L924).
    //
    //  So, the number of components in a given cluster is dependent on `direction`.
    //   - In LTR, `num_components` is the difference between the next cluster and the current cluster.
    //   - In RTL, `num_components` is the difference between the last cluster and the current cluster.
    // This is because we must compare the current cluster to its next larger ID (in other words, the next
    // logical index, which is visually downstream in LTR and visually upstream in RTL).
    //
    // For example, consider the LTR text for "afi" where "fi" form a ligature.
    //   Initial cluster values: 0, 1, 2 (logical + visual order)
    //   `HarfRust` assignation: 0, 1, 1
    //   Cluster count:          2
    //   `num_components`:       (1 - 0 =) 1, (3 - 1 =) 2
    //
    // Now consider the RTL text for "حداً".
    //   Initial cluster values:  0, 1, 2, 3 (logical, or in-memory, order)
    //   Reversed cluster values: 3, 2, 1, 0 (visual order - the return order of `HarfRust` for RTL)
    //   `HarfRust` assignation:  3, 2, 0, 0
    //   Cluster count:           3
    //   `num_components`:        (4 - 3 =) 1, (3 - 2 =) 1, (2 - 0 =) 2
    let num_components =
        |next_cluster: u32, current_cluster: u32, last_cluster: u32| match direction {
            Direction::Ltr => next_cluster - current_cluster,
            Direction::Rtl => last_cluster - current_cluster,
        };
    let mut last_cluster_id: u32 = match direction {
        Direction::Ltr => 0,
        Direction::Rtl => char_infos.len() as u32,
    };

    for (glyph_info, glyph_pos) in glyph_infos.iter().zip(glyph_positions.iter()) {
        // Flush previous cluster if we've reached a new cluster
        if cluster_id != glyph_info.cluster {
            run_advance += cluster_advance;
            let num_components = num_components(glyph_info.cluster, cluster_id, last_cluster_id);
            cluster_advance /= num_components as f32;
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            let cluster_type = if num_components > 1 {
                debug_assert!(!is_newline);
                ClusterType::LigatureStart
            } else if is_newline {
                ClusterType::Newline
            } else {
                ClusterType::Regular
            };

            let inline_glyph_id = if matches!(cluster_type, ClusterType::Regular) {
                pending_inline_glyph.take().map(|g| g.id)
            } else {
                // This isn't a regular cluster, so we don't inline the glyph and push
                // it to `glyphs`.
                if let Some(pending) = pending_inline_glyph.take() {
                    glyphs.push(pending);
                    total_glyphs += 1;
                }
                None
            };

            if num_components > 1 && direction == Direction::Rtl {
                push_rtl_ligature(
                    clusters,
                    char_infos,
                    cluster_id,
                    num_components,
                    cluster_start_char,
                    &mut char_indices_iter,
                    cluster_glyph_offset,
                    cluster_advance,
                    total_glyphs,
                );
                cluster_glyph_offset = total_glyphs;
            } else {
                push_cluster(
                    clusters,
                    char_info,
                    cluster_start_char,
                    cluster_glyph_offset,
                    cluster_advance,
                    total_glyphs,
                    cluster_type,
                    inline_glyph_id,
                );
                cluster_glyph_offset = total_glyphs;
            }

            if num_components > 1 && direction == Direction::Ltr {
                // Skip characters until we reach the current cluster
                for i in 1..num_components {
                    cluster_start_char = char_indices_iter.next().unwrap();
                    if to_whitespace(cluster_start_char.1) == Whitespace::Space {
                        break;
                    }
                    let char_info_ = match direction {
                        Direction::Ltr => char_infos[(cluster_id + i) as usize],
                        Direction::Rtl => char_infos[(cluster_id + num_components - i) as usize],
                    };
                    push_cluster(
                        clusters,
                        char_info_,
                        cluster_start_char,
                        cluster_glyph_offset,
                        cluster_advance,
                        total_glyphs,
                        ClusterType::LigatureComponent,
                        None,
                    );
                }
            }
            cluster_start_char = char_indices_iter.next().unwrap();

            cluster_advance = 0.0;
            last_cluster_id = cluster_id;
            cluster_id = glyph_info.cluster;
            char_info = char_infos[cluster_id as usize];
            pending_inline_glyph = None;
        }

        let glyph = Glyph {
            id: glyph_info.glyph_id,
            style_index: char_info.1,
            x: (glyph_pos.x_offset as f32) * scale_factor,
            // Convert from font space (Y-up) to layout space (Y-down)
            y: -(glyph_pos.y_offset as f32) * scale_factor,
            advance: (glyph_pos.x_advance as f32) * scale_factor,
        };
        cluster_advance += glyph.advance;
        // Push any pending glyph. If it was a zero-offset, single glyph cluster, it would
        // have been pushed in the first `if` block.
        if let Some(pending) = pending_inline_glyph.take() {
            glyphs.push(pending);
            total_glyphs += 1;
        }
        if total_glyphs == cluster_glyph_offset && glyph.x == 0.0 && glyph.y == 0.0 {
            // Defer this potential zero-offset, single glyph cluster
            pending_inline_glyph = Some(glyph);
        } else {
            glyphs.push(glyph);
            total_glyphs += 1;
        }
    }

    // Push the last cluster
    {
        // See comment above `num_components` for why we use `char_infos.len()` for LTR and 0 for RTL.
        let next_cluster_id = match direction {
            Direction::Ltr => char_infos.len() as u32,
            Direction::Rtl => 0,
        };
        let num_components = num_components(next_cluster_id, cluster_id, last_cluster_id);
        if num_components > 1 {
            // This is a ligature - create ligature start + ligature components

            if let Some(pending) = pending_inline_glyph.take() {
                glyphs.push(pending);
                total_glyphs += 1;
            }
            let ligature_advance = cluster_advance / num_components as f32;
            if direction == Direction::Rtl {
                push_rtl_ligature(
                    clusters,
                    char_infos,
                    cluster_id,
                    num_components,
                    cluster_start_char,
                    &mut char_indices_iter,
                    cluster_glyph_offset,
                    ligature_advance,
                    total_glyphs,
                );
                return run_advance;
            }
            push_cluster(
                clusters,
                char_info,
                cluster_start_char,
                cluster_glyph_offset,
                ligature_advance,
                total_glyphs,
                ClusterType::LigatureStart,
                None,
            );

            cluster_glyph_offset = total_glyphs;

            // Create ligature component clusters for the remaining characters
            for (i, char) in (1..).zip(char_indices_iter) {
                if to_whitespace(char.1) == Whitespace::Space {
                    break;
                }
                let component_char_info = match direction {
                    Direction::Ltr => char_infos[(cluster_id + i) as usize],
                    Direction::Rtl => char_infos[(cluster_id + num_components - i) as usize],
                };
                push_cluster(
                    clusters,
                    component_char_info,
                    char,
                    cluster_glyph_offset,
                    ligature_advance,
                    total_glyphs,
                    ClusterType::LigatureComponent,
                    None,
                );
            }
        } else {
            let is_newline = to_whitespace(cluster_start_char.1) == Whitespace::Newline;
            let cluster_type = if is_newline {
                ClusterType::Newline
            } else {
                ClusterType::Regular
            };
            let mut inline_glyph_id = None;
            match cluster_type {
                ClusterType::Regular => {
                    if total_glyphs == cluster_glyph_offset {
                        if let Some(pending) = pending_inline_glyph.take() {
                            inline_glyph_id = Some(pending.id);
                        }
                    }
                }
                _ => {
                    if let Some(pending) = pending_inline_glyph.take() {
                        glyphs.push(pending);
                        total_glyphs += 1;
                    }
                }
            }
            push_cluster(
                clusters,
                char_info,
                cluster_start_char,
                cluster_glyph_offset,
                cluster_advance,
                total_glyphs,
                cluster_type,
                inline_glyph_id,
            );
        }
    }

    run_advance
}

#[derive(PartialEq)]
enum Direction {
    Ltr,
    Rtl,
}

enum ClusterType {
    LigatureStart,
    LigatureComponent,
    Regular,
    Newline,
}

impl From<&ClusterType> for u16 {
    fn from(cluster_type: &ClusterType) -> Self {
        match cluster_type {
            ClusterType::LigatureStart => ClusterData::LIGATURE_START,
            ClusterType::LigatureComponent => ClusterData::LIGATURE_COMPONENT,
            ClusterType::Regular | ClusterType::Newline => 0, // No special flags
        }
    }
}

/// Pushes a right-to-left cluster of several characters (a base and its
/// marks, or a ligature). The source characters run backwards for RTL, so
/// `last_char` is the cluster's logically last character and `chars` yields
/// the rest, last to first. The run's clusters are reversed into logical order
/// afterwards, so this pushes the components first (last to first) and the
/// ligature start last: after the reversal the start comes first, where the
/// line breaker and `Run` expect it, and each character carries its own
/// `CharInfo`. Pairing the start's info with the last character instead put
/// the cluster's line boundary inside it (between a letter and its mark) and
/// lost the boundary before it.
fn push_rtl_ligature<I: Iterator<Item = (usize, char)>>(
    clusters: &mut Vec<ClusterData>,
    char_infos: &[(CharInfo, u16)],
    cluster_id: u32,
    num_components: u32,
    last_char: (usize, char),
    chars: &mut core::iter::Peekable<I>,
    glyph_offset: u32,
    advance: f32,
    total_glyphs: u32,
) {
    let mut ch = last_char;
    for k in 0..num_components - 1 {
        if to_whitespace(ch.1) != Whitespace::Space {
            let index = cluster_id + num_components - 1 - k;
            push_cluster(
                clusters,
                char_infos[index as usize],
                ch,
                total_glyphs,
                advance,
                total_glyphs,
                ClusterType::LigatureComponent,
                None,
            );
        }
        ch = chars.next().unwrap();
    }
    push_cluster(
        clusters,
        char_infos[cluster_id as usize],
        ch,
        glyph_offset,
        advance,
        total_glyphs,
        ClusterType::LigatureStart,
        None,
    );
}

fn push_cluster(
    clusters: &mut Vec<ClusterData>,
    char_info: (CharInfo, u16),
    cluster_start_char: (usize, char),
    glyph_offset: u32,
    advance: f32,
    total_glyphs: u32,
    cluster_type: ClusterType,
    inline_glyph_id: Option<u32>,
) {
    let glyph_len = (total_glyphs - glyph_offset) as u8;

    let (final_glyph_len, final_glyph_offset, final_advance) = match cluster_type {
        ClusterType::LigatureComponent => {
            // Ligature components have no glyphs, only advance.
            debug_assert_eq!(glyph_len, 0);
            (0_u8, 0_u32, advance)
        }
        ClusterType::Newline => {
            // Newline clusters are stripped of their glyph contribution.
            debug_assert_eq!(glyph_len, 1);
            (0_u8, 0_u32, 0.0)
        }
        _ if inline_glyph_id.is_some() => {
            // Inline glyphs are stored inline within `ClusterData`
            debug_assert_eq!(glyph_len, 0);
            (0xFF_u8, inline_glyph_id.unwrap(), advance)
        }
        ClusterType::Regular | ClusterType::LigatureStart => {
            // Regular and ligature start clusters maintain their glyphs and advance.
            debug_assert_ne!(glyph_len, 0);
            (glyph_len, glyph_offset, advance)
        }
    };

    clusters.push(ClusterData {
        info: ClusterInfo::new(char_info.0.boundary, cluster_start_char.1),
        flags: (&cluster_type).into(),
        style_index: char_info.1,
        glyph_len: final_glyph_len,
        text_len: cluster_start_char.1.len_utf8() as u8,
        glyph_offset: final_glyph_offset,
        text_offset: cluster_start_char.0 as u16,
        advance: final_advance,
    });
}
