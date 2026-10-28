//! Viewport placement from ExactKit/PositionArea.swift's `origin` path.
//! Each box is placed at its viewport-fit size; attempts do not remeasure it.
use crate::paint::Rect4;
use exact_kernel::{Dimension, Direction, Env, StyleId, StyleProps};

const FALLBACKS: &str = "flip-block, flip-inline, flip-block flip-inline";
type Margins = [f32; 4];

#[derive(Default)]
pub(in crate::presenter) struct History {
    choice: usize,
    inputs: Option<Options>,
}
#[derive(PartialEq)]
struct Options {
    area: String,
    fallbacks: String,
    rtl: bool,
    margins: Margins,
    size: (f32, f32),
}
impl History {
    fn place(&mut self, options: Options, anchor: Rect4, viewport: (f32, f32)) -> (f32, f32) {
        // ExactKit resets on size and style changes; moving the anchor or
        // resizing the viewport keeps the last successful option (CSS §6.5).
        if self.inputs.as_ref() != Some(&options) {
            self.choice = 0;
        }
        let point = place(&options, anchor, viewport, &mut self.choice);
        self.inputs = Some(options);
        point
    }
}

pub(in crate::presenter) fn at_point(
    point: (f32, f32),
    size: (f32, f32),
    viewport: (f32, f32),
) -> (f32, f32) {
    (
        point.0.min(viewport.0 - size.0).max(0.),
        point.1.min(viewport.1 - size.1).max(0.),
    )
}

/// Where a popover's authored `left` and `top` put its border box in a
/// viewport `viewport` big (LLP 1021 D2): past its margin, as CSS places a
/// fixed box by its insets; an axis with none at its margin. `None` when it
/// writes neither, and its invoker places it; `None` too under an authored
/// `position-area`, which CSS lets win. A host presents it as it presents a
/// context menu at that point.
pub(in crate::presenter) fn inset_point(
    style: &StyleProps,
    env: &Env,
    viewport: (f32, f32),
) -> Option<(f32, f32)> {
    if style.position_area.name() != "none" {
        return None;
    }
    let written = |d: Dimension| (d.resolve(env) != Dimension::Auto).then_some(d);
    let (left, top) = (written(style.left), written(style.top));
    if left.is_none() && top.is_none() {
        return None;
    }
    let at = |inset: Option<Dimension>, margin: Dimension, basis: f32| {
        inset.map_or(0., |d| d.at_basis(env, basis)) + margin.at_basis(env, viewport.0)
    };
    Some((
        at(left, style.margin_left, viewport.0),
        at(top, style.margin_top, viewport.1),
    ))
}

pub(in crate::presenter) fn origin(
    style: &StyleProps,
    env: &Env,
    anchor: Rect4,
    size: (f32, f32),
    viewport: (f32, f32),
    history: &mut History,
) -> (f32, f32) {
    let rtl = style.direction == Direction::Rtl;
    let said = style.mask.union(style.written);
    let area = if said.has(StyleId::PositionArea) {
        style.position_area.name()
    } else if rtl {
        "bottom span-left"
    } else {
        "bottom span-right"
    };
    let fallbacks = if said.has(StyleId::PositionTryFallbacks) {
        style.position_try_fallbacks.css()
    } else if said.has(StyleId::PositionArea) {
        "none".into()
    } else {
        FALLBACKS.into()
    };
    // As on Apple, unresolved percentages and auto margins contribute zero.
    let margins = [
        style.margin_top,
        style.margin_right,
        style.margin_bottom,
        style.margin_left,
    ]
    .map(|d| match d.resolve(env) {
        Dimension::Points(n) => n,
        _ => 0.,
    });
    history.place(
        Options {
            area: area.into(),
            fallbacks,
            rtl,
            margins,
            size,
        },
        anchor,
        viewport,
    )
}

fn axes(area: &str) -> (i8, i8) {
    if area == "none" {
        return (1, 2);
    }
    if area == "center" {
        return (3, 3);
    }
    let (mut x, mut y) = (0, 0);
    for word in area.split_ascii_whitespace() {
        match word {
            "left" => x = -2,
            "right" => x = 2,
            "top" => y = -2,
            "bottom" => y = 2,
            "span-left" => x = -1,
            "span-right" => x = 1,
            "span-top" => y = -1,
            "span-bottom" => y = 1,
            _ => {}
        }
    }
    (x, y)
}

fn flipped(option: &str, (mut x, mut y): (i8, i8), mut m: Margins, rtl: bool) -> (i8, i8, Margins) {
    for word in option.split_ascii_whitespace() {
        match word {
            "flip-block" => {
                y = -y;
                m.swap(0, 2);
            }
            "flip-inline" => {
                x = -x;
                m.swap(1, 3);
            }
            "flip-start" if rtl => {
                (x, y) = (-y, -x);
                m = [m[1], m[0], m[3], m[2]];
            }
            "flip-start" => {
                std::mem::swap(&mut x, &mut y);
                m = [m[3], m[2], m[1], m[0]];
            }
            _ => {}
        }
    }
    (x, y, m)
}

fn at(side: i8, low: f32, high: f32, length: f32, end: f32) -> f32 {
    match side {
        -2 => low - length,
        -1 => high - length,
        1 => low,
        2 => high,
        _ => ((low + high - length) / 2.).min(end - length).max(0.),
    }
}

fn overflowed(
    rect: Rect4,
    point: (f32, f32),
    sides: (i8, i8),
    anchor: Rect4,
    rtl: bool,
    viewport: (f32, f32),
) -> (f32, f32) {
    fn limit(side: i8, low: f32, high: f32, end: f32) -> (f32, f32) {
        let (min, max) = (0f32.min(low), end.max(high));
        let area = match side {
            -2 => (min, low),
            -1 => (min, high),
            1 => (low, max),
            2 => (high, max),
            _ => (min, max),
        };
        (0f32.min(area.0), end.max(area.1))
    }
    fn shifted(at: f32, length: f32, range: (f32, f32), start_high: bool) -> f32 {
        if start_high {
            at.max(range.0).min(range.1 - length)
        } else {
            at.min(range.1 - length).max(range.0)
        }
    }
    let x = shifted(
        rect.0,
        rect.2,
        limit(sides.0, anchor.0, anchor.0 + anchor.2, viewport.0),
        rtl,
    );
    let y = shifted(
        rect.1,
        rect.3,
        limit(sides.1, anchor.1, anchor.1 + anchor.3, viewport.1),
        false,
    );
    (point.0 + x - rect.0, point.1 + y - rect.1)
}

fn place(options: &Options, anchor: Rect4, viewport: (f32, f32), choice: &mut usize) -> (f32, f32) {
    let Options {
        area,
        fallbacks,
        rtl,
        margins,
        size,
    } = options;
    let (rtl, margins) = (*rtl, *margins);
    let axes = axes(area);
    let attempt = |option| {
        let (x, y, m) = flipped(option, axes, margins, rtl);
        let outer = (size.0 + m[1] + m[3], size.1 + m[0] + m[2]);
        let mut point = (
            at(x, anchor.0, anchor.0 + anchor.2, outer.0, viewport.0),
            at(y, anchor.1, anchor.1 + anchor.3, outer.1, viewport.1),
        );
        if area == "none" {
            point = at_point(point, outer, viewport);
        }
        (
            (point.0 + m[3], point.1 + m[0]),
            (point.0, point.1, outer.0, outer.1),
            (x, y),
        )
    };
    let options: Vec<_> = std::iter::once("none")
        .chain(fallbacks.split(','))
        .collect();
    let previous = *choice;
    let current = attempt(options[previous]);
    for index in std::iter::once(previous).chain((0..options.len()).filter(|i| *i != previous)) {
        let (point, b, _) = attempt(options[index]);
        if area == "none"
            || b.0 >= 0. && b.1 >= 0. && b.0 + b.2 <= viewport.0 && b.1 + b.3 <= viewport.1
        {
            *choice = index;
            return point;
        }
    }
    overflowed(current.1, current.0, current.2, anchor, rtl, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(
        area: &str,
        fallbacks: &str,
        rtl: bool,
        margins: Margins,
        anchor: Rect4,
        size: (f32, f32),
        viewport: (f32, f32),
    ) -> (f32, f32) {
        History::default().place(
            Options {
                area: area.into(),
                fallbacks: fallbacks.into(),
                rtl,
                margins,
                size,
            },
            anchor,
            viewport,
        )
    }

    #[test]
    fn last_successful_option_survives_more_room_but_resets_for_size_and_style() {
        let mut history = History::default();
        let env = Env::default();
        let mut style = StyleProps::default();
        let anchor = (100., 170., 40., 20.);
        assert_eq!(
            origin(&style, &env, anchor, (80., 50.), (300., 200.), &mut history),
            (100., 120.)
        );
        assert_eq!(history.choice, 1);
        assert_eq!(
            origin(
                &style,
                &env,
                (100., 20., 40., 20.),
                (80., 50.),
                (300., 60.),
                &mut history
            ),
            (100., 0.)
        );
        assert_eq!(history.choice, 1);
        assert_eq!(
            origin(&style, &env, anchor, (80., 50.), (300., 300.), &mut history),
            (100., 120.)
        );
        assert_eq!(
            origin(&style, &env, anchor, (80., 60.), (300., 300.), &mut history),
            (100., 190.)
        );
        assert_eq!(history.choice, 0);
        assert_eq!(
            origin(&style, &env, anchor, (80., 60.), (300., 200.), &mut history),
            (100., 110.)
        );
        style.margin_top = Dimension::Points(4.);
        assert_eq!(
            origin(&style, &env, anchor, (80., 60.), (300., 300.), &mut history),
            (100., 194.)
        );
        assert_eq!(history.choice, 0);
    }

    #[test]
    fn defaults_authored_areas_margins_and_context_points() {
        let env = Env::default();
        let mut s = StyleProps {
            margin_top: Dimension::Points(4.),
            ..Default::default()
        };
        assert_eq!(
            origin(
                &s,
                &env,
                (100., 80., 40., 20.),
                (80., 50.),
                (300., 200.),
                &mut History::default()
            ),
            (100., 104.)
        );
        s.direction = Direction::Rtl;
        assert_eq!(
            origin(
                &s,
                &env,
                (100., 80., 40., 20.),
                (80., 50.),
                (300., 200.),
                &mut History::default()
            ),
            (60., 104.)
        );
        s.position_area = exact_kernel::PositionArea::TopSpanLeft;
        s.mask.set(StyleId::PositionArea);
        s.margin_bottom = Dimension::Points(6.);
        assert_eq!(
            origin(
                &s,
                &env,
                (100., 80., 40., 20.),
                (80., 50.),
                (300., 200.),
                &mut History::default()
            ),
            (60., 24.)
        );
        assert_eq!(
            at_point((280., 190.), (80., 50.), (300., 200.)),
            (220., 150.)
        );
    }

    #[test]
    fn attempts_flip_the_base_in_order_and_keep_margins() {
        let bounds = (300., 200.);
        let size = (80., 50.);
        assert_eq!(
            place(
                "bottom span-right",
                FALLBACKS,
                false,
                [4., 3., 6., 2.],
                (250., 170., 40., 20.),
                size,
                bounds
            ),
            (208., 116.)
        );
        assert_eq!(
            flipped("flip-start flip-block", (1, 2), [1., 2., 3., 4.], false),
            (2, -1, [2., 3., 4., 1.])
        );
        assert_eq!(
            flipped("flip-block flip-start", (1, 2), [1., 2., 3., 4.], false),
            (-2, 1, [4., 1., 2., 3.])
        );
        assert_eq!(
            flipped("flip-start", (1, 2), [1., 2., 3., 4.], true),
            (-2, -1, [2., 1., 4., 3.])
        );
        assert_eq!(
            place(
                "center",
                "none",
                false,
                [0.; 4],
                (140., 90., 20., 20.),
                size,
                bounds
            ),
            (110., 75.)
        );
        assert_eq!(
            place(
                "right",
                "none",
                false,
                [0.; 4],
                (280., 80., 20., 20.),
                size,
                bounds
            ),
            (220., 65.)
        );
        assert_eq!(
            place(
                "center",
                "none",
                true,
                [0.; 4],
                (140., 90., 20., 20.),
                (400., 250.),
                bounds
            ),
            (-100., 0.)
        );
    }
}
