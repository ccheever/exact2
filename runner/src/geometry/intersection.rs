//! Geometry shared by AppKit, UIKit and Linux, using the same transformed
//! boxes and reported scroll offsets as `frame()` (LLP 1051.000).
use super::*;

type Rect = [f64; 4]; // left, top, right, bottom

pub(crate) fn intersection(
    kernel: &Kernel,
    scrolled: &Scrolled,
    view: ViewId,
) -> (Option<NodeKey>, String, bool, f64) {
    let arena = kernel.arena();
    let key = arena.key_of(view).expect("a mounted handler");
    let margin = arena
        .props(key.index)
        .str(PropId::IntersectMargin)
        .unwrap_or("0px")
        .to_owned();
    // The nearest scroller on the steps the target is shown through, the
    // ones the mapping below takes (`shown_in`): none above a box its host
    // lifts to the top layer, and none a placed block's root steps past
    let mut root = None;
    let mut at = key.index;
    while let Shown::Parent(slot, _) = shown_in(kernel, scrolled, at, arena.frame(at)) {
        let s = arena.style(slot);
        let implicit_scroll =
            !s.mask.has(StyleId::OverflowY) && arena.node_type(slot).scrolls_by_default();
        if implicit_scroll || s.overflow_x != Overflow::Visible || s.overflow_y != Overflow::Visible
        {
            root = Some(arena.key(slot));
            break;
        }
        at = slot;
    }
    let answer = || -> Option<(bool, f64)> {
        let (frame, _) = kernel.laid_out_frame(key)?;
        let mut ancestor = Some(key.index);
        while let Some(slot) = ancestor {
            if arena.style(slot).display == Display::None
                || arena.flags(slot).has(NodeFlags::HIDDEN)
            {
                return None;
            }
            ancestor = arena.parent(slot);
        }
        let target = kernel_frame(kernel, scrolled, view);
        if target.unavailable {
            return None;
        }
        let area = target.width * target.height;
        let mut clipped = [0.0, 0.0, f64::from(frame.width), f64::from(frame.height)];
        let mut slot = key.index;
        let mut transform = IDENTITY;
        loop {
            let s = arena.style(slot);
            if s.display == Display::None || arena.flags(slot).has(NodeFlags::HIDDEN) {
                return None;
            }
            let f = arena.frame(slot);
            if root.is_some_and(|r| r.index == slot) {
                let border = |width, style| match style {
                    exact_kernel::BorderStyle::None | exact_kernel::BorderStyle::Hidden => 0.0,
                    _ => f64::from(width),
                };
                let bounds = [
                    border(s.border_width_left, s.border_style_left),
                    border(s.border_width_top, s.border_style_top),
                    f64::from(f.width) - border(s.border_width_right, s.border_style_right),
                    f64::from(f.height) - border(s.border_width_bottom, s.border_style_bottom),
                ];
                let bounds = expand(bounds, &margin);
                clipped = clip(map(transform, clipped), bounds)?;
                // Ratio is measured in viewport coordinates, as is the target.
                let visible = to_viewport(kernel, scrolled, slot, clipped);
                return Some((true, visible.map_or(0.0, |v| ratio(v, area))));
            }
            if slot != key.index {
                if let Some(bounds) = path_bounds(&s.rare.clip_path) {
                    clipped = clip(map(transform, clipped), bounds)?;
                    transform = IDENTITY;
                }
            }
            transform = mul(placed_transform(kernel, scrolled, slot), transform);
            // A block clips what its host shows in it, a sheet or a pane
            if let Some(bounds) = block_clip(kernel, slot, f) {
                clipped = clip(map(transform, clipped), bounds)?;
                transform = IDENTITY;
            }
            match shown_in(kernel, scrolled, slot, f) {
                Shown::Parent(up, to) => {
                    transform = mul(to, transform);
                    slot = up;
                }
                Shown::Viewport(to) => {
                    clipped = map(mul(to, transform), clipped);
                    let env = arena.env();
                    let viewport = [
                        0.0,
                        0.0,
                        f64::from(env.viewport_width),
                        f64::from(env.viewport_height),
                    ];
                    clipped = clip(clipped, expand(viewport, &margin))?;
                    return Some((true, ratio(clipped, area)));
                }
            }
        }
    };
    let (inside, ratio) = answer().unwrap_or((false, 0.0));
    (root, margin, inside, ratio)
}

/// `rect`, in `slot`'s local coordinates, where it shows in the viewport,
/// less what the blocks above it clip; `None` when they clip all of it.
fn to_viewport(
    kernel: &Kernel,
    scrolled: &Scrolled,
    mut slot: u32,
    mut rect: Rect,
) -> Option<Rect> {
    let arena = kernel.arena();
    let mut transform = IDENTITY;
    loop {
        let f = arena.frame(slot);
        transform = mul(placed_transform(kernel, scrolled, slot), transform);
        if let Some(bounds) = block_clip(kernel, slot, f) {
            rect = clip(map(transform, rect), bounds)?;
            transform = IDENTITY;
        }
        match shown_in(kernel, scrolled, slot, f) {
            Shown::Parent(up, to) => {
                transform = mul(to, transform);
                slot = up;
            }
            Shown::Viewport(to) => return Some(map(mul(to, transform), rect)),
        }
    }
}

fn placed_transform(kernel: &Kernel, scrolled: &Scrolled, slot: u32) -> Affine {
    let arena = kernel.arena();
    let own = own_transform(arena.style(slot), arena.frame(slot));
    let Some(constraint) = kernel.sticky_constraint(arena.key(slot)) else {
        return own;
    };
    let Some(root) = arena.key_of(constraint.scroller) else {
        return own;
    };
    let s = arena.style(root.index);
    let implicit =
        !s.mask.has(StyleId::OverflowY) && arena.node_type(root.index).scrolls_by_default();
    let scroll =
        if implicit || s.overflow_x != Overflow::Visible || s.overflow_y != Overflow::Visible {
            scrolled.offset(root)
        } else if arena.is_root(root.index) {
            let frame = arena.frame(root.index);
            (
                scrolled.page.0 - f64::from(frame.x),
                scrolled.page.1 - f64::from(frame.y),
            )
        } else {
            (0.0, 0.0)
        };
    // Sticky placement is outside the box's own transform, as in the presenters.
    let (x, y) = constraint.offset((scroll.0 as f32, scroll.1 as f32));
    mul(shift(f64::from(x), f64::from(y)), own)
}

fn map(m: Affine, r: Rect) -> Rect {
    [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])]
        .into_iter()
        .map(|(x, y)| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]))
        .fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |r, (x, y)| [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)],
        )
}
fn clip(a: Rect, b: Rect) -> Option<Rect> {
    let r = [
        a[0].max(b[0]),
        a[1].max(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ];
    (r.iter().all(|x| x.is_finite()) && r[0] <= r[2] && r[1] <= r[3]).then_some(r)
}
fn ratio(r: Rect, area: f64) -> f64 {
    if area == 0.0 {
        1.0
    } else {
        (((r[2] - r[0]) * (r[3] - r[1])) / area).clamp(0.0, 1.0)
    }
}
fn expand(r: Rect, text: &str) -> Rect {
    let lengths: Vec<_> = text
        .split_whitespace()
        .map(|word| {
            let percentage = word.ends_with('%');
            let number = word
                .strip_suffix('%')
                .or_else(|| word.strip_suffix("px"))
                .unwrap_or(word);
            (exact_num::parse_f64(number).unwrap_or(0.0), percentage)
        })
        .collect();
    let sides = match lengths.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => [(0.0, false); 4],
    };
    // Chrome's rootMargin resolves vertical percentages against height,
    // horizontal ones against width (LLP 1001's recorded spec deviation).
    let basis = [r[3] - r[1], r[2] - r[0], r[3] - r[1], r[2] - r[0]];
    let sides = std::array::from_fn::<_, 4, _>(|i| {
        let (value, percentage) = sides[i];
        if percentage {
            value * basis[i] / 100.0
        } else {
            value
        }
    });
    [
        r[0] - sides[3],
        r[1] - sides[0],
        r[2] + sides[1],
        r[3] + sides[2],
    ]
}

fn path_bounds(path: &exact_kernel::clip::ClipPath) -> Option<Rect> {
    use exact_kernel::svg::{Path, Seg};
    let segments = path
        .commands()
        .iter()
        .filter_map(|(op, v)| match (*op, v.as_slice()) {
            ('M', [x, y]) => Some(Seg::Move(*x, *y)),
            ('L', [x, y]) => Some(Seg::Line(*x, *y)),
            ('C', [a, b, c, d, x, y]) => Some(Seg::Cubic(*a, *b, *c, *d, *x, *y)),
            ('Z', []) => Some(Seg::Close),
            _ => None,
        })
        .collect();
    Path(segments).bounds().map(|(x, y, w, h)| {
        [
            f64::from(x),
            f64::from(y),
            f64::from(x + w),
            f64::from(y + h),
        ]
    })
}
