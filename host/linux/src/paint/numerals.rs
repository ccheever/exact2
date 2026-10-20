//! @ref LLP 1077 D15 — a paragraph under `-exact-content-transition:
//! numeric` rolls the glyphs that changed, as SwiftUI's numeric text does on
//! Apple (LLP 1115.003 D3): when the number rose, the new glyph comes down
//! from above out of a blur as the old one sinks, blurs and fades (the other
//! way when it fell, and always for `numeric-countdown`), each changed glyph
//! a beat after the one to its left; a glyph that stays slides to its new
//! place. Glyphs pair by place from the right within each number, so `999` →
//! `1,000` changes three digits and brings in `1,`. Each glyph is the
//! paragraph drawn clipped to its advance, so every backend rolls with the
//! clip and text it already has; the blur is the backend's where it blurs
//! glyphs cheaply (Android's), else none. The painter samples it every frame
//! until it ends; Reduce Motion, a new paragraph, more than one line or
//! right-to-left text changes in place.

use super::*;
use exact_kernel::{ContentTransition, NodeKey};
use std::collections::HashSet;

/// How long one glyph rolls, ms: Apple's smooth spring (`.spring(duration:
/// 0.4, bounce: 0)`, the Apple capability's) is within 0.4% of its end here.
const DURATION: f64 = 500.0;
/// How long after the changed glyph to its left a glyph starts, ms.
const STAGGER: f64 = 50.0;
/// How far a glyph travels, and the blur it starts or ends in (a sigma), in
/// its font size.
const TRAVEL: f32 = 0.5;
const BLUR: f32 = 0.05;

#[derive(Default)]
pub(crate) struct Numerals {
    rolls: HashMap<NodeKey, Roll>,
    /// Rolls the walk painted: the rest end with the paint.
    painted: HashSet<NodeKey>,
    /// The paint's clock, ms.
    pub now: f64,
    /// When the latest commit since the last paint landed, ms: a roll starts
    /// with the commit that changed its text, however late the paint.
    pub committed: Option<f64>,
    /// Reduce Motion.
    pub still: bool,
}

#[derive(Clone)]
pub(super) struct Roll {
    from: Rc<Paragraph>,
    to: Rc<Paragraph>,
    start: f64,
    /// Whether the new glyphs come down from above.
    rising: bool,
    /// How long it plays, ms: its last glyph's start and [`DURATION`].
    span: f64,
}

impl Numerals {
    /// Whether a roll owes a frame.
    pub(crate) fn rolling(&self) -> bool {
        !self.rolls.is_empty()
    }

    /// When the last roll ends, ms.
    pub(crate) fn settles_at(&self) -> Option<f64> {
        self.rolls
            .values()
            .map(|r| r.start + r.span)
            .reduce(f64::max)
    }

    /// After a paint: a roll the walk did not paint has ended.
    pub(super) fn painted(&mut self) {
        let painted = std::mem::take(&mut self.painted);
        self.rolls.retain(|key, _| painted.contains(key));
    }

    /// The roll `to` shows now and how far into it, ms, if one plays: begun
    /// when its text changed from `from`, the paragraph the last frame showed.
    pub(super) fn roll(
        &mut self,
        key: NodeKey,
        mode: ContentTransition,
        from: Option<&Rc<Paragraph>>,
        to: &Rc<Paragraph>,
    ) -> Option<(Roll, f64)> {
        if mode == ContentTransition::None || self.still {
            self.rolls.remove(&key);
            return None;
        }
        let changed = from.filter(|from| !Rc::ptr_eq(from, to) && text(from) != text(to));
        if let Some(from) = changed.filter(|from| rolls(from) && rolls(to)) {
            let rising = mode == ContentTransition::Numeric
                && leading_number(&text(to)) >= leading_number(&text(from));
            let turns = motions(&cells(from), &cells(to))
                .iter()
                .filter(|m| m.turn.is_some())
                .count();
            let roll = Roll {
                from: from.clone(),
                to: to.clone(),
                start: self.committed.unwrap_or(self.now).min(self.now),
                rising,
                span: DURATION + STAGGER * turns.saturating_sub(1) as f64,
            };
            self.rolls.insert(key, roll);
        } else if changed.is_some() {
            self.rolls.remove(&key);
        }
        let roll = self.rolls.get_mut(&key)?;
        // A paragraph at a new width keeps the roll's text.
        if !Rc::ptr_eq(&roll.to, to) {
            roll.to = to.clone();
        }
        let elapsed = self.now - roll.start;
        if elapsed >= roll.span {
            self.rolls.remove(&key);
            return None;
        }
        self.painted.insert(key);
        Some((roll.clone(), elapsed.max(0.0)))
    }
}

/// The spring's progress `ms` into a glyph's roll: a critically damped
/// spring of Apple's 0.4 s response, 1 from [`DURATION`] on.
fn spring(ms: f64) -> f32 {
    if ms >= DURATION {
        return 1.0;
    }
    let w = std::f64::consts::TAU / 0.4 * (ms.max(0.0) / 1000.0);
    (1.0 - (1.0 + w) * (-w).exp()) as f32
}

fn text(p: &Paragraph) -> String {
    p.layout_runs().map(|line| line.text).collect()
}

/// Whether a paragraph's glyphs can roll: one left-to-right line.
fn rolls(p: &Paragraph) -> bool {
    let mut lines = p.layout_runs();
    lines.next().is_some_and(|line| !line.rtl) && lines.next().is_none()
}

/// What pairs a glyph with the one it replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Place {
    /// In the `number`th number from the right, `place` characters from its
    /// right.
    Digit { number: usize, place: usize },
    /// Before the first number, `place` characters from the start.
    Lead { place: usize },
    /// After the `number`th number from the right, `place` characters on.
    After { number: usize, place: usize },
}

/// Each character's place in `text`.
fn places(text: &str) -> Vec<Place> {
    let chars: Vec<char> = text.chars().collect();
    let digit = |i: usize| chars.get(i).is_some_and(|c| c.is_ascii_digit());
    // A group or decimal separator between two digits is the number's.
    let numeric = |i: usize| {
        digit(i)
            || (matches!(chars[i], ',' | '.' | '\u{a0}' | '\u{202f}')
                && i > 0
                && digit(i - 1)
                && digit(i + 1))
    };
    let mut numbers: Vec<(usize, usize)> = Vec::new();
    for i in 0..chars.len() {
        if !numeric(i) {
            continue;
        }
        match numbers.last_mut() {
            Some((_, end)) if *end == i => *end = i + 1,
            _ => numbers.push((i, i + 1)),
        }
    }
    let count = numbers.len();
    (0..chars.len())
        .map(|i| {
            // The first number that ends after `i`: `i` is in it or before it.
            let next = numbers.iter().position(|&(_, end)| i < end);
            match next {
                Some(n) if i >= numbers[n].0 => Place::Digit {
                    number: count - 1 - n,
                    place: numbers[n].1 - 1 - i,
                },
                Some(0) => Place::Lead { place: i },
                None if count == 0 => Place::Lead { place: i },
                Some(n) => Place::After {
                    number: count - n,
                    place: i - numbers[n - 1].1,
                },
                None => Place::After {
                    number: 0,
                    place: i - numbers[count - 1].1,
                },
            }
        })
        .collect()
}

/// The number a text shows first, its group separators skipped and a sign
/// before it kept; 0 when it shows none.
fn leading_number(text: &str) -> f64 {
    let Some(first) = text.find(|c: char| c.is_ascii_digit()) else {
        return 0.0;
    };
    let negative = text[..first].contains(['-', '\u{2212}']);
    let mut digits = String::from(if negative { "-" } else { "" });
    let mut dot = false;
    for c in text[first..].chars() {
        match c {
            '0'..='9' => digits.push(c),
            ',' => {}
            '.' if !dot => {
                dot = true;
                digits.push(c);
            }
            _ => break,
        }
    }
    digits.trim_end_matches('.').parse().unwrap_or(0.0)
}

/// One cluster's glyphs on a paragraph's line: its place, characters, and
/// advance from the paragraph's left.
struct Cell {
    place: Place,
    chars: String,
    x: f32,
    w: f32,
}

fn cells(p: &Paragraph) -> Vec<Cell> {
    let Some(line) = p.layout_runs().next() else {
        return Vec::new();
    };
    let places = places(line.text);
    let mut cells: Vec<(usize, Cell)> = Vec::new();
    for g in line.glyphs {
        match cells.last_mut() {
            Some((start, cell)) if *start == g.start as usize => cell.w += g.w,
            _ => {
                let at = line.text[..g.start as usize].chars().count();
                let Some(place) = places.get(at) else {
                    continue;
                };
                let chars = line.text.get(g.range()).unwrap_or("").to_string();
                let cell = Cell {
                    place: *place,
                    chars,
                    x: g.x,
                    w: g.w,
                };
                cells.push((g.start as usize, cell));
            }
        }
    }
    cells.into_iter().map(|(_, c)| c).collect()
}

/// What one place does: a glyph that stays (both cells, the same
/// characters), one replaced (both), one that leaves or one that comes; left
/// to right, a changed one's turn in the stagger.
#[derive(Debug, PartialEq)]
struct Motion {
    from: Option<usize>,
    to: Option<usize>,
    turn: Option<usize>,
}

fn motions(from: &[Cell], to: &[Cell]) -> Vec<Motion> {
    let mut out: Vec<(f32, Motion)> = Vec::new();
    for (i, cell) in to.iter().enumerate() {
        let was = from.iter().position(|c| c.place == cell.place);
        let to = Some(i);
        out.push((
            cell.x,
            Motion {
                from: was,
                to,
                turn: None,
            },
        ));
    }
    for (j, cell) in from.iter().enumerate() {
        if !to.iter().any(|c| c.place == cell.place) {
            let from = Some(j);
            out.push((
                cell.x,
                Motion {
                    from,
                    to: None,
                    turn: None,
                },
            ));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut turn = 0;
    for (_, m) in &mut out {
        let stays = matches!((m.from, m.to), (Some(j), Some(i)) if from[j].chars == to[i].chars);
        if !stays {
            m.turn = Some(turn);
            turn += 1;
        }
    }
    out.into_iter().map(|(_, m)| m).collect()
}

/// One glyph to draw: its paragraph, that paragraph's paint, its cells, and
/// which cell.
type Glyph<'a> = (&'a Paragraph, &'a [RunPaint], &'a [Cell], usize);

impl Painter {
    /// `roll`, `elapsed` ms in, its paragraphs' glyphs at `origin`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn roll_glyphs(
        &mut self,
        node: &NodeRef<'_>,
        kernel: &Kernel,
        (roll, elapsed): &(Roll, f64),
        palette: &[RunPaint],
        origin: (f32, f32),
        rect: Rect4,
        ts: Transform,
        reveal: Option<ViewId>,
    ) {
        let size = roll.to.runs().first().map_or(16.0, |r| r.size);
        // Down from above when the number rose: the new glyph's offset at
        // its start, the old one's at its end.
        let travel = size * TRAVEL * if roll.rising { 1.0 } else { -1.0 };
        // The old paragraph's runs take the new one's paint, run by run.
        let fallback = palette.last().copied();
        let old_paint: Vec<RunPaint> = (0..roll.from.runs().len())
            .filter_map(|i| palette.get(i).copied().or(fallback))
            .collect();
        let (from, to) = (cells(&roll.from), cells(&roll.to));
        let reach = (size * 2.0, roll.to.width.max(roll.from.width) + size * 2.0);
        let draw = |painter: &mut Self, glyph: Glyph<'_>, (dx, dy): (f32, f32), alpha: f32| {
            let (paragraph, paint, cells, i) = glyph;
            if alpha <= 0.0 || paint.len() < paragraph.runs().len() {
                return;
            }
            let cell = &cells[i];
            // The outermost cells reach past the paragraph, for overhang.
            let left = if i == 0 { -reach.0 } else { cell.x };
            let right = if i + 1 == cells.len() {
                reach.1
            } else {
                cell.x + cell.w
            };
            let clip = Shape::rect((
                origin.0 + dx + left,
                origin.1 + dy - size,
                right - left,
                paragraph.height + size * 2.0,
            ));
            let faded: Vec<RunPaint> = paint
                .iter()
                .map(|r| {
                    let [red, green, blue, a] = r.color;
                    let a = (a as f32 * alpha).round() as u8;
                    RunPaint {
                        color: [red, green, blue, a],
                        ..*r
                    }
                })
                .collect();
            painter.backend.push_clip(&clip, ts);
            // Sharp where it rests: blurred as it leaves or comes.
            painter.backend.text_blur(size * BLUR * (1.0 - alpha));
            let at = (origin.0 + dx, origin.1 + dy);
            painter.text_paint(node, kernel, paragraph, &faded, at, rect, ts, reveal);
            painter.backend.text_blur(0.0);
            painter.backend.pop_clip();
        };
        // Glyphs that stay slide over the whole roll.
        let slide = 1.0 - spring(*elapsed * DURATION / roll.span);
        for m in motions(&from, &to) {
            let p = m
                .turn
                .map_or(1.0, |k| spring(*elapsed - STAGGER * k as f64));
            let (new, old) = (
                |i| (&*roll.to, palette, &to[..], i),
                |j| (&*roll.from, &old_paint[..], &from[..], j),
            );
            match (m.from, m.to) {
                (Some(j), Some(i)) => {
                    // Where the pair stands now, between its two places.
                    let apart = from[j].x - to[i].x;
                    let shift = apart * slide;
                    if m.turn.is_some() {
                        draw(
                            self,
                            old(j),
                            (shift - apart, travel * p),
                            (1.0 - p) * (1.0 - p),
                        );
                    }
                    draw(self, new(i), (shift, -travel * (1.0 - p)), p);
                }
                (None, Some(i)) => draw(self, new(i), (0.0, -travel * (1.0 - p)), p),
                (Some(j), None) => draw(self, old(j), (0.0, travel * p), (1.0 - p) * (1.0 - p)),
                (None, None) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digit(number: usize, place: usize) -> Place {
        Place::Digit { number, place }
    }

    #[test]
    fn digits_pair_by_place_from_the_right() {
        // "999" and "1,000": the three digits pair, the "1" and "," are new.
        let (a, b) = (places("999"), places("1,000"));
        assert_eq!(a, vec![digit(0, 2), digit(0, 1), digit(0, 0)]);
        assert_eq!(
            b,
            vec![
                digit(0, 4),
                digit(0, 3),
                digit(0, 2),
                digit(0, 1),
                digit(0, 0)
            ]
        );
    }

    #[test]
    fn a_prefix_and_a_suffix_stay_put() {
        let p = places("$12.50 +3%");
        assert_eq!(p[0], Place::Lead { place: 0 });
        assert_eq!(
            &p[1..6],
            &[
                digit(1, 4),
                digit(1, 3),
                digit(1, 2),
                digit(1, 1),
                digit(1, 0)
            ]
        );
        assert_eq!(
            p[6],
            Place::After {
                number: 1,
                place: 0
            }
        );
        assert_eq!(
            p[7],
            Place::After {
                number: 1,
                place: 1
            }
        );
        assert_eq!(p[8], digit(0, 0));
        assert_eq!(
            p[9],
            Place::After {
                number: 0,
                place: 0
            }
        );
        // A full stop that ends a sentence is not the number's.
        assert_eq!(
            places("7.")[1],
            Place::After {
                number: 0,
                place: 0
            }
        );
        assert_eq!(
            places("ab"),
            vec![Place::Lead { place: 0 }, Place::Lead { place: 1 }]
        );
    }

    #[test]
    fn the_leading_number_says_which_way_it_rolls() {
        assert_eq!(leading_number("1,234.5 builds"), 1234.5);
        assert_eq!(leading_number("$999"), 999.0);
        assert_eq!(leading_number("\u{2212}3 today"), -3.0);
        assert_eq!(leading_number("7."), 7.0);
        assert_eq!(leading_number("none"), 0.0);
    }

    #[test]
    fn the_spring_ends_at_its_duration() {
        assert_eq!(spring(0.0), 0.0);
        assert!(spring(200.0) > 0.8, "most of the way at 200 ms");
        assert!(1.0 - spring(DURATION - 1.0) < 0.004);
        assert_eq!(spring(DURATION), 1.0);
    }

    fn row(text: &str) -> Vec<Cell> {
        let places = places(text);
        let cell = |(i, c): (usize, char)| Cell {
            place: places[i],
            chars: c.into(),
            x: i as f32 * 10.0,
            w: 10.0,
        };
        text.chars().enumerate().map(cell).collect()
    }

    #[test]
    fn changed_glyphs_take_turns_left_to_right() {
        // "$999" → "$1,000": "$" stays; "1" and "," come; the 9s change.
        let turns: Vec<_> = motions(&row("$999"), &row("$1,000"))
            .iter()
            .map(|m| (m.from, m.to, m.turn))
            .collect();
        assert_eq!(
            turns,
            vec![
                (Some(0), Some(0), None),
                (None, Some(1), Some(0)),
                (None, Some(2), Some(1)),
                (Some(1), Some(3), Some(2)),
                (Some(2), Some(4), Some(3)),
                (Some(3), Some(5), Some(4)),
            ]
        );
        // "12" → "7": the "2" becomes "7" and the "1" leaves, both in turn.
        let ms = motions(&row("12"), &row("7"));
        let pairs: Vec<_> = ms.iter().map(|m| (m.from, m.to)).collect();
        assert_eq!(pairs, vec![(Some(1), Some(0)), (Some(0), None)]);
        assert!(ms.iter().all(|m| m.turn.is_some()));
    }
}

#[cfg(test)]
#[path = "numerals_tests.rs"]
mod presenter_tests;
