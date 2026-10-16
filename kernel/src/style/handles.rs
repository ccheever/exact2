//! Interned calculation handles for Taffy, resolved against the current layout
//! environment and percentage basis.

use super::{Comparison, Env};

/// The `calc()` pairs the engine holds by handle: Taffy keeps one opaque
/// pointer per length, so a pair is interned here and named by its index,
/// shifted past the three tag bits Taffy reserves. Equal pairs share a
/// handle, so an unchanged style still compares equal by value. `floor`
/// clamps the result at zero for a row with no negative values (CSS Values 4
/// §10.12); `added` is added after the clamp.
static CALC_PAIRS: std::sync::Mutex<Vec<CalcPair>> = std::sync::Mutex::new(Vec::new());

#[derive(Clone, Copy)]
struct CalcPair {
    percent: f32,
    points: f32,
    floor: bool,
    added: f32,
}

pub(super) fn calc_handle(percent: f32, points: f32) -> *const () {
    calc_handle_in(percent, points, false, 0.0)
}

pub(crate) fn calc_handle_in(percent: f32, points: f32, floor: bool, added: f32) -> *const () {
    handle(intern_pair(CalcPair {
        percent,
        points,
        floor,
        added,
    }))
}

/// A handle names a pair or a tree by an index whose low bit says which,
/// shifted past the three tag bits Taffy reserves.
fn handle(index: usize) -> *const () {
    ((index + 1) << 3) as *const ()
}

fn intern_pair(pair: CalcPair) -> usize {
    let CalcPair {
        percent,
        points,
        floor,
        added,
    } = pair;
    let same = |p: &CalcPair| {
        p.percent.to_bits() == percent.to_bits()
            && p.points.to_bits() == points.to_bits()
            && p.floor == floor
            && p.added.to_bits() == added.to_bits()
    };
    let mut pairs = CALC_PAIRS.lock().unwrap_or_else(|e| e.into_inner());
    let index = pairs.iter().position(same).unwrap_or_else(|| {
        pairs.push(pair);
        pairs.len() - 1
    });
    index << 1
}

/// The trees with a percentage the engine holds by handle: a handle names
/// the tree, not resolved numbers, and the environment is the layout's own
/// ([`LayoutEnv`]), so a resize resolves the same handles again rather than
/// minting new ones.
static TREES: std::sync::Mutex<Vec<TreeCalc>> = std::sync::Mutex::new(Vec::new());

#[derive(Clone, Copy, PartialEq)]
struct TreeCalc {
    tree: Comparison,
    floor: bool,
    added: u32,
}

/// The engine's handle for a tree with a percentage, floored at zero when
/// `floor`, plus `added` after.
pub(crate) fn tree_handle(tree: Comparison, floor: bool, added: f32) -> *const () {
    let entry = TreeCalc {
        tree,
        floor,
        added: added.to_bits(),
    };
    let mut trees = TREES.lock().unwrap_or_else(|e| e.into_inner());
    let index = trees.iter().position(|t| *t == entry).unwrap_or_else(|| {
        trees.push(entry);
        trees.len() - 1
    });
    handle(index << 1 | 1)
}

std::thread_local! {
    /// The environment of the layout running on this thread.
    static LAYOUT_ENV: std::cell::RefCell<Env> = std::cell::RefCell::new(Env::default());
}

/// The environment for resolving tree handles during layout or host-block
/// lowering. Dropping the guard restores the previous environment.
pub(crate) struct LayoutEnv(Option<Env>);

impl LayoutEnv {
    pub(crate) fn set(env: &Env) -> LayoutEnv {
        LayoutEnv(Some(LAYOUT_ENV.with(|e| e.replace(env.clone()))))
    }
}

impl Drop for LayoutEnv {
    fn drop(&mut self) {
        if let Some(before) = self.0.take() {
            LAYOUT_ENV.with(|e| *e.borrow_mut() = before);
        }
    }
}

/// The points a `calc()` handle resolves to against `basis`, the size its
/// percentage is a fraction of — what [`crate::LayoutTree`] gives Taffy.
/// The arithmetic is `f64`, narrowed once at the end, so a pair and a tree
/// holding the same calculation round alike.
pub(crate) fn resolve_calc(handle: *const (), basis: f32) -> f32 {
    let index = (handle as usize >> 3) - 1;
    let basis = f64::from(basis);
    let (value, floor, added) = if index & 1 == 1 {
        let tree = TREES.lock().unwrap_or_else(|e| e.into_inner())[index >> 1];
        let value = LAYOUT_ENV.with(|e| tree.tree.wide(&e.borrow(), Some(basis)));
        (value.unwrap_or(0.0), tree.floor, f32::from_bits(tree.added))
    } else {
        let pair = CALC_PAIRS.lock().unwrap_or_else(|e| e.into_inner())[index >> 1];
        let value = basis * f64::from(pair.percent) / 100.0 + f64::from(pair.points);
        (value, pair.floor, pair.added)
    };
    // A NaN is 0 before the clamp, as a narrowed one would be
    let value = if value.is_nan() { 0.0 } else { value };
    super::calc::narrow((if floor { value.max(0.0) } else { value }) + f64::from(added))
}
