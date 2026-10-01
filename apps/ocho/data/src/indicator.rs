//! Fleet-colored working indicators (status_indicator.rs): Claude's
//! breathing glyph and Codex's shimmer, as numbers and text the view paints.
//! Pure: the phase comes from `now_ms` on the host clock.

use crate::theme::{Rgba, Theme};

/// One breath of the Claude glyph, and one shimmer pass, in milliseconds.
pub const INDICATOR_CYCLE_MS: f64 = 2000.0;

// The non-Claude shimmer needs a 30 fps repaint the view does not run; the header shows unanimated.
#[allow(dead_code)]
/// How often a view showing an indicator asks for a new frame (≈30 fps): a
/// 30 fps tick is indistinguishable for a two-second cosine and costs a
/// quarter of the renders.
pub const INDICATOR_TICK_MS: f64 = 33.0;

/// Where in the cycle `now_ms` falls, 0 ≤ φ < 1.
pub fn phase(now_ms: f64) -> f64 {
    let cycle = INDICATOR_CYCLE_MS;
    (now_ms.rem_euclid(cycle)) / cycle
}

/// Claude's six glyphs, quietest first.
pub const CLAUDE_GLYPHS: [&str; 6] = ["·", "✢", "✳", "✶", "✻", "✽"];

/// Claude Code 2.1.266 eases through these six glyphs with a cosine over two
/// seconds. Rounding naturally holds both extremes for about 410 ms per cycle.
pub fn claude_glyph(phase: f64) -> &'static str {
    // f32 as the desktop computes it, so the hold boundaries land the same.
    let phase = phase as f32;
    let expansion = (1.0 - (std::f32::consts::TAU * phase).cos()) / 2.0;
    CLAUDE_GLYPHS[(expansion * (CLAUDE_GLYPHS.len() - 1) as f32).round() as usize]
}

/// What a Claude status line says about the turn, which picks its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaudeStatusTone {
    /// "Working", or any header with an ellipsis: warn.
    Working,
    /// A "Compacting…" header: ansi[4] (blue).
    Compacting,
    /// Anything else (waiting on a background agent, a retry): plain text.
    Neutral,
}

/// The tone of a Claude status text, judged by its header (before " (").
pub fn claude_status_tone(text: &str) -> ClaudeStatusTone {
    let header = text.split(" (").next().unwrap_or(text).trim();
    if header.to_ascii_lowercase().starts_with("compacting") {
        ClaudeStatusTone::Compacting
    } else if header.eq_ignore_ascii_case("working")
        || header.contains('…')
        || header.contains("...")
    {
        ClaudeStatusTone::Working
    } else {
        ClaudeStatusTone::Neutral
    }
}

/// Claude's compaction row carries its native progress bar as "… 57%" (see
/// core's pane parser). The sidebar shows just "Compacting", a bar and the
/// percent.
pub fn compaction_progress(text: &str) -> Option<u8> {
    if claude_status_tone(text) != ClaudeStatusTone::Compacting {
        return None;
    }
    let header = &text[..text.find(" (").unwrap_or(text.len())];
    let (_, percent) = header.trim_end().rsplit_once(' ')?;
    let value: u8 = percent.strip_suffix('%')?.parse().ok()?;
    (value <= 100).then_some(value)
}

/// The shimmer's center character for a header of `len` characters at
/// `phase`: `φ·(len+6) − 3`, so the ±3 window enters from the left and
/// leaves on the right.
pub fn shimmer_center(len: usize, phase: f64) -> f64 {
    phase * (len as f64 + 6.0) - 3.0
}

// The non-Claude shimmer needs a 30 fps repaint the view does not run; the header shows unanimated.
#[allow(dead_code)]
/// How much of the highlight character `index` gets, 0..=1, for a shimmer
/// centered at `center`.
pub fn shimmer_amount(index: usize, center: f64) -> f64 {
    (1.0 - (index as f64 - center).abs() / 3.0).max(0.0)
}

// The non-Claude shimmer needs a 30 fps repaint the view does not run; the header shows unanimated.
#[allow(dead_code)]
/// The color of character `index` in a `len`-character header at `phase`,
/// blending `base` toward `highlight` (status_indicator.rs `theme_shimmer`).
pub fn shimmer_color(base: Rgba, highlight: Rgba, len: usize, index: usize, phase: f64) -> Rgba {
    let amount = shimmer_amount(index, shimmer_center(len, phase));
    let mix = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount).round() as u8;
    Rgba(
        mix(base.0, highlight.0),
        mix(base.1, highlight.1),
        mix(base.2, highlight.2),
        base.3 + (highlight.3 - base.3) * amount,
    )
}

/// A working indicator, ready to paint.
#[derive(Clone, Debug, PartialEq)]
pub struct Indicator {
    /// True for Claude: a glyph precedes the text.
    pub claude: bool,
    /// The glyph, empty for other providers.
    pub glyph: String,
    /// The text before the last " (": the header.
    pub head: String,
    /// The header's color as CSS: Claude's tone color, or the shimmer's base
    /// (`theme.text`) for others, who shimmer it toward `theme.accent`.
    pub head_color: String,
    /// The rest of the text, from the " (" on; empty when the status was
    /// folded into a compaction bar.
    pub rest: String,
    /// The rest's color: `theme.text` for Claude, `theme.muted` otherwise.
    pub rest_color: String,
    /// The shimmer highlight color (`theme.accent`), for non-Claude rows.
    pub shimmer_color: String,
    /// The shimmer's center character index, for non-Claude rows.
    pub shimmer_center: f64,
    /// The header's length in characters (the shimmer window's `len`).
    pub head_len: usize,
    /// Claude's compaction percent, when the status carried one: the view
    /// shows a bar and "NN%" in the header color after "Compacting".
    pub progress: Option<u8>,
    /// The tone, for Claude.
    pub tone: ClaudeStatusTone,
    /// The phase the indicator was computed at.
    pub phase: f64,
}

/// The indicator for `provider`'s status `text` at `now_ms`
/// (status_indicator.rs `working_indicator`, minus the elements).
pub fn indicator(provider: &str, text: &str, now_ms: f64, theme: &Theme) -> Indicator {
    let claude = provider == "claude";
    let progress = compaction_progress(text).filter(|_| claude);
    let text: String = if progress.is_some() {
        "Compacting".to_string()
    } else {
        text.to_string()
    };
    let header_end = text.rfind(" (").unwrap_or(text.len());
    let head = text[..header_end].to_string();
    let rest = text[header_end..].to_string();
    let head_len = head.chars().count();
    let tone = claude_status_tone(&text);
    let phase = phase(now_ms);
    let (head_color, rest_color) = if claude {
        let head_color = match tone {
            ClaudeStatusTone::Working => theme.warn,
            ClaudeStatusTone::Compacting => theme.ansi[4],
            // The ordinary text color is intentionally brighter than `muted`.
            // Waiting statuses must stay legible against the sidebar surface.
            ClaudeStatusTone::Neutral => theme.text,
        };
        (head_color, theme.text)
    } else {
        (theme.text, theme.muted)
    };
    Indicator {
        claude,
        glyph: if claude {
            claude_glyph(phase).to_string()
        } else {
            String::new()
        },
        head,
        head_color: head_color.css(),
        rest,
        rest_color: rest_color.css(),
        shimmer_color: theme.accent.css(),
        shimmer_center: shimmer_center(head_len, phase),
        head_len,
        progress,
        tone,
        phase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_status_tones_are_owned_by_fleet() {
        for text in [
            "Working (1s)",
            "Thinking… (1s)",
            "Generating... (12s)",
            "Reticulating…",
        ] {
            assert_eq!(
                claude_status_tone(text),
                ClaudeStatusTone::Working,
                "{text}"
            );
        }
        assert_eq!(
            claude_status_tone("Compacting conversation… (1m 2s)"),
            ClaudeStatusTone::Compacting
        );
        for text in [
            "Waiting for 1 background agent to finish",
            "API error · Retrying in 36s · attempt 7/10",
        ] {
            assert_eq!(
                claude_status_tone(text),
                ClaudeStatusTone::Neutral,
                "{text}"
            );
        }
    }

    #[test]
    fn compaction_percent_becomes_a_bar() {
        assert_eq!(
            compaction_progress("Compacting conversation… 57% (10m 36s · ↓ 27.5k tokens)"),
            Some(57)
        );
        assert_eq!(
            compaction_progress("Compacting conversation… 100%"),
            Some(100)
        );
        for text in [
            "Compacting conversation… (1m 2s)",
            "Compacting conversation… 157% (1m 2s)",
            "Thinking… 57% (1m 2s)",
        ] {
            assert_eq!(compaction_progress(text), None, "{text}");
        }
    }

    #[test]
    fn claude_breathes_over_two_seconds_with_holds_at_both_extremes() {
        assert_eq!(INDICATOR_CYCLE_MS, 2000.0);
        // Sample both sides of each hold and the expansion/contraction. An
        // evenly spaced ping-pong sequence loses the native cadence here.
        for (ms, expected) in [
            (0, "·"),
            (200, "·"),
            (220, "✢"),
            (300, "✢"),
            (400, "✳"),
            (520, "✶"),
            (700, "✻"),
            (780, "✻"),
            (800, "✽"),
            (1000, "✽"),
            (1200, "✽"),
            (1220, "✻"),
            (1300, "✻"),
            (1480, "✶"),
            (1600, "✳"),
            (1700, "✢"),
            (1780, "✢"),
            (1800, "·"),
            (2000, "·"),
        ] {
            let phase = ms as f64 / INDICATOR_CYCLE_MS;
            assert_eq!(claude_glyph(phase), expected, "at {ms} ms");
            // The host clock wraps the same way, whatever cycle it is in.
            assert_eq!(
                indicator(
                    "claude",
                    "Working…",
                    6000.0 + ms as f64,
                    &Theme::ocho_dark()
                )
                .glyph,
                expected
            );
        }
    }

    #[test]
    fn claude_indicator_splits_head_and_colors_by_tone() {
        let t = Theme::ocho_dark();
        let i = indicator("claude", "Thinking… (1s · ↓ 134 tokens)", 0.0, &t);
        assert!(i.claude);
        assert_eq!(i.glyph, "·");
        assert_eq!(i.head, "Thinking…");
        assert_eq!(i.rest, " (1s · ↓ 134 tokens)");
        assert_eq!(i.head_color, t.warn.css());
        assert_eq!(i.rest_color, t.text.css());
        assert_eq!(i.progress, None);

        let i = indicator("claude", "Compacting conversation… 57% (10m)", 500.0, &t);
        assert_eq!(i.head, "Compacting");
        assert_eq!(i.rest, "");
        assert_eq!(i.progress, Some(57));
        assert_eq!(i.head_color, t.ansi[4].css());
        assert_eq!(i.tone, ClaudeStatusTone::Compacting);

        let i = indicator(
            "claude",
            "Waiting for 1 background agent to finish",
            0.0,
            &t,
        );
        assert_eq!(i.head_color, t.text.css());
        assert_eq!(i.rest, "");
    }

    #[test]
    fn codex_shimmers_the_header() {
        let t = Theme::ocho_dark();
        let i = indicator("codex", "Running tests (12s)", 0.0, &t);
        assert!(!i.claude);
        assert_eq!(i.glyph, "");
        assert_eq!(i.head, "Running tests");
        assert_eq!(i.head_len, 13);
        assert_eq!(i.rest, " (12s)");
        assert_eq!(i.head_color, t.text.css());
        assert_eq!(i.rest_color, t.muted.css());
        assert_eq!(i.shimmer_center, -3.0);
        // Compaction percentages are Claude's; Codex keeps its text.
        let i = indicator("codex", "Compacting conversation… 57%", 1000.0, &t);
        assert_eq!(i.progress, None);
        assert_eq!(i.head, "Compacting conversation… 57%");
        assert_eq!(i.shimmer_center, 0.5 * (28.0 + 6.0) - 3.0);
    }

    #[test]
    fn shimmer_window_is_three_characters_wide() {
        assert_eq!(shimmer_center(10, 0.0), -3.0);
        assert_eq!(shimmer_center(10, 1.0), 13.0);
        assert_eq!(shimmer_amount(5, 5.0), 1.0);
        assert_eq!(shimmer_amount(5, 6.5), 0.5);
        assert_eq!(shimmer_amount(5, 8.0), 0.0);
        assert_eq!(shimmer_amount(5, 9.0), 0.0);
        let base = Rgba(0, 0, 0, 1.0);
        let hi = Rgba(200, 100, 0, 1.0);
        assert_eq!(
            shimmer_color(base, hi, 10, 2, (2.0 + 3.0) / 16.0),
            Rgba(200, 100, 0, 1.0)
        );
        assert_eq!(shimmer_color(base, hi, 10, 9, 0.0), base);
    }

    #[test]
    fn phase_wraps_the_host_clock() {
        assert_eq!(phase(0.0), 0.0);
        assert_eq!(phase(500.0), 0.25);
        assert_eq!(phase(2000.0), 0.0);
        assert_eq!(phase(4500.0), 0.25);
        assert_eq!(phase(-500.0), 0.75);
    }
}
