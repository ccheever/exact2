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
    let mut root = None;
    let mut at = arena.parent(key.index);
    while let Some(slot) = at {
        let s = arena.style(slot);
        let implicit_scroll =
            !s.mask.has(StyleId::OverflowY) && arena.node_type(slot).scrolls_by_default();
        if implicit_scroll || s.overflow_x != Overflow::Visible || s.overflow_y != Overflow::Visible
        {
            root = Some(arena.key(slot));
            break;
        }
        at = arena.parent(slot);
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
                return Some((true, ratio(visible, area)));
            }
            if slot != key.index {
                if let Some(bounds) = path_bounds(&s.rare.clip_path) {
                    clipped = clip(map(transform, clipped), bounds)?;
                    transform = IDENTITY;
                }
            }
            transform = mul(own_transform(s, f), transform);
            let parent = (!arena.is_root(slot)).then(|| arena.parent(slot)).flatten();
            let Some(up) = parent else {
                clipped = map(
                    mul(
                        shift(
                            f64::from(f.x) - scrolled.page.0,
                            f64::from(f.y) - scrolled.page.1,
                        ),
                        transform,
                    ),
                    clipped,
                );
                let env = arena.env();
                clipped = clip(
                    clipped,
                    expand(
                        [
                            0.0,
                            0.0,
                            f64::from(env.viewport_width),
                            f64::from(env.viewport_height),
                        ],
                        &margin,
                    ),
                )?;
                return Some((true, ratio(clipped, area)));
            };
            let p = arena.frame(up);
            let (left, top) = scrolled.offset(arena.key(up));
            transform = mul(
                shift(f64::from(f.x - p.x) - left, f64::from(f.y - p.y) - top),
                transform,
            );
            slot = up;
        }
    };
    let (inside, ratio) = answer().unwrap_or((false, 0.0));
    (root, margin, inside, ratio)
}

fn to_viewport(kernel: &Kernel, scrolled: &Scrolled, mut slot: u32, rect: Rect) -> Rect {
    let arena = kernel.arena();
    let mut transform = IDENTITY;
    loop {
        let f = arena.frame(slot);
        transform = mul(own_transform(arena.style(slot), f), transform);
        let Some(up) = (!arena.is_root(slot)).then(|| arena.parent(slot)).flatten() else {
            return map(
                mul(
                    shift(
                        f64::from(f.x) - scrolled.page.0,
                        f64::from(f.y) - scrolled.page.1,
                    ),
                    transform,
                ),
                rect,
            );
        };
        let p = arena.frame(up);
        let (left, top) = scrolled.offset(arena.key(up));
        transform = mul(
            shift(f64::from(f.x - p.x) - left, f64::from(f.y - p.y) - top),
            transform,
        );
        slot = up;
    }
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
