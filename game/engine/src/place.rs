//! Deterministic placement in meters, Y-up and −Z forward.
//! Generators are lazy and allocate nothing except `scatter`, which reserves at
//! most `count` points. Invalid/nonfinite dimensions panic. Seeded helpers use a
//! private engine RNG and do not advance the world's RNG.
use crate::{math, spatial, Mesh, Quat, Rng, Target, Transform, Vec3, World};

fn nonnegative(n: f32) {
    assert!(
        n.is_finite() && n >= 0.0,
        "expected finite nonnegative dimension"
    );
}
fn polar(angle: f32, radius: f32) -> Vec3 {
    Vec3::new(math::cos(angle) * radius, 0.0, math::sin(angle) * radius)
}

/// Equally spaced XZ points, starting at +X and proceeding toward +Z.
/// Zero count is empty; one point is at `(radius, 0, 0)`.
/// ```
/// use exact_game::{place, Vec3};
/// assert_eq!(place::ring(1, 2.0).collect::<Vec<_>>(), [Vec3::X * 2.0]);
/// ```
pub fn ring(count: usize, radius: f32) -> impl ExactSizeIterator<Item = Vec3> {
    nonnegative(radius);
    (0..count).map(move |i| polar(i as f32 * std::f32::consts::TAU / count as f32, radius))
}

/// One point per equal angular sector of an XZ annulus. Angle and radius are
/// uniform within their intervals (radius is not area-weighted); inner may equal outer.
/// ```
/// use exact_game::place;
/// let a: Vec<_> = place::ring_jittered(7, 8, 2.0, 3.0).collect();
/// assert_eq!(a, place::ring_jittered(7, 8, 2.0, 3.0).collect::<Vec<_>>());
/// assert!(a.iter().all(|p| p.length() >= 2.0 && p.length() <= 3.0));
/// ```
pub fn ring_jittered(
    seed: u64,
    count: usize,
    inner: f32,
    outer: f32,
) -> impl ExactSizeIterator<Item = Vec3> {
    nonnegative(inner);
    nonnegative(outer);
    assert!(inner <= outer);
    let mut rng = Rng::new(seed);
    (0..count).map(move |i| {
        let angle = (i as f32 + rng.next_f32()) * std::f32::consts::TAU / count as f32;
        polar(angle, math::lerp(inner, outer, rng.next_f32()))
    })
}

/// XZ grid centered on zero, row-major (+X columns, +Z rows). Spacing is the
/// distance between centers. Panics if cols × rows overflows usize.
/// ```
/// use exact_game::{place, Vec3};
/// assert_eq!(place::grid(2, 1, 2.0).collect::<Vec<_>>(), [-Vec3::X, Vec3::X]);
/// ```
pub fn grid(cols: usize, rows: usize, spacing: f32) -> impl ExactSizeIterator<Item = Vec3> {
    nonnegative(spacing);
    let count = cols.checked_mul(rows).expect("grid size overflow");
    (0..count).map(move |i| {
        Vec3::new(
            (i.rem_euclid(cols) as f32 - (cols as f32 - 1.0) * 0.5) * spacing,
            0.0,
            (i / cols) as f32 * spacing - (rows as f32 - 1.0) * 0.5 * spacing,
        )
    })
}

/// Evenly spaced points including both endpoints. Zero count is empty; one
/// point is `a`. The last point is exactly `b`.
/// ```
/// use exact_game::{place, Vec3};
/// assert_eq!(place::line(Vec3::ZERO, Vec3::X, 3).collect::<Vec<_>>(),
///     [Vec3::ZERO, Vec3::X * 0.5, Vec3::X]);
/// ```
pub fn line(a: Vec3, b: Vec3, count: usize) -> impl ExactSizeIterator<Item = Vec3> {
    assert!(a.is_finite() && b.is_finite());
    (0..count).map(move |i| {
        if i == 0 {
            a
        } else if i == count - 1 {
            b
        } else {
            a.lerp(b, i as f32 / (count - 1) as f32)
        }
    })
}

/// Rejection sampling in an axis-aligned 3D box; equal bounds permit planes or
/// points. Reserves `count` points, tries at most 64 candidates per requested
/// point, and returns a shorter vector on exhaustion (no spacing relaxation).
/// Work is O(count²); distance is inclusive and measured in 3D.
/// ```
/// use exact_game::{place, Vec3};
/// let points = place::scatter(7, Vec3::ZERO, Vec3::ZERO, 3, 1.0);
/// assert_eq!(points, [Vec3::ZERO]); // impossible packing returns a partial result
/// ```
pub fn scatter(seed: u64, min: Vec3, max: Vec3, count: usize, min_distance: f32) -> Vec<Vec3> {
    assert!(min.is_finite() && max.is_finite() && min.cmple(max).all());
    nonnegative(min_distance);
    let mut rng = Rng::new(seed);
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        for _ in 0..64 {
            let p = Vec3::new(
                math::lerp(min.x, max.x, rng.next_f32()),
                math::lerp(min.y, max.y, rng.next_f32()),
                math::lerp(min.z, max.z, rng.next_f32()),
            );
            if points
                .iter()
                .all(|q: &Vec3| p.distance_squared(*q) >= min_distance * min_distance)
            {
                points.push(p);
                break;
            }
        }
    }
    points
}

/// Aim −Z from `from` to `to`, keeping +Y up. Coincident points give identity;
/// vertical directions use +Z as the roll reference. No allocation.
/// ```
/// use exact_game::{place, Vec3};
/// let q = place::facing(Vec3::ZERO, Vec3::X);
/// assert!((q * -Vec3::Z - Vec3::X).length() < 1e-6);
/// ```
pub fn facing(from: Vec3, to: Vec3) -> Quat {
    assert!(from.is_finite() && to.is_finite());
    if from == to {
        return Quat::IDENTITY;
    }
    let direction = (to - from).normalize();
    let up = if direction.cross(Vec3::Y).length_squared() < 1e-12 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    Transform::at(from.x, from.y, from.z)
        .looking_at(to, up)
        .rotation
}

fn bounds(world: &World, target: impl Target) -> (Vec3, Vec3) {
    let entity = target
        .entity(world)
        .expect("placement target does not exist");
    let pose = world
        .current_global(entity)
        .expect("placement target has no global pose");
    let mesh = world.get::<Mesh>(entity);
    let corners = spatial::corners(
        pose,
        spatial::extent(mesh.as_deref()),
        spatial::center(mesh.as_deref()),
    );
    corners.into_iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(min, max), p| (min.min(p), max.max(p)),
    )
}

/// Center an unparented, axis-aligned object above the target's world-space
/// bounds, using its full height. Uses layout's mesh bounds (including planes'
/// downward slab), current rotation, scale and parents (no propagation needed); meshless targets are points.
/// Panics for a missing target/pose. Does not mutate the world or allocate.
/// ```
/// use exact_game::{place, World, Mesh, Transform, Vec3};
/// let mut w = World::new(60, 0);
/// w.spawn_named("base", (Transform::default(), Mesh::cube(2.0)));
/// assert_eq!(place::on_top_of(&w, "base", 1.0), Vec3::new(0.0, 1.5, 0.0));
/// ```
pub fn on_top_of(world: &World, target: impl Target, own_height: f32) -> Vec3 {
    nonnegative(own_height);
    let (min, max) = bounds(world, target);
    Vec3::new(
        (min.x + max.x) * 0.5,
        max.y + own_height * 0.5,
        (min.z + max.z) * 0.5,
    )
}

/// World-axis side for [`next_to`]; front follows the engine's −Z forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// −X.
    Left,
    /// +X.
    Right,
    /// −Z.
    Front,
    /// +Z.
    Back,
}

/// Center an unparented, axis-aligned object beside the target's world bounds.
/// `own_size` is its full world size; centers align on the other axes. Gap must
/// be nonnegative. Uses the same bounds and missing-target policy as `on_top_of`.
/// ```
/// use exact_game::{place::{self, Side}, World, Mesh, Transform, Vec3};
/// let mut w = World::new(60, 0);
/// w.spawn_named("base", (Transform::default(), Mesh::cube(2.0)));
/// assert_eq!(place::next_to(&w, "base", Side::Right, 0.5, Vec3::ONE), Vec3::X * 2.0);
/// ```
pub fn next_to(world: &World, target: impl Target, side: Side, gap: f32, own_size: Vec3) -> Vec3 {
    nonnegative(gap);
    for n in own_size.to_array() {
        nonnegative(n);
    }
    let (min, max) = bounds(world, target);
    let mut at = (min + max) * 0.5;
    let (axis, positive) = match side {
        Side::Left => (0, false),
        Side::Right => (0, true),
        Side::Front => (2, false),
        Side::Back => (2, true),
    };
    at[axis] = if positive {
        max[axis] + gap + own_size[axis] * 0.5
    } else {
        min[axis] - gap - own_size[axis] * 0.5
    };
    at
}

/// Invalid ASCII blockout, with one-based row/column coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockoutError {
    /// Row containing the error.
    pub row: usize,
    /// First missing/extra cell, or non-ASCII character's column.
    pub column: usize,
    /// Description of the error.
    pub reason: &'static str,
}
impl std::fmt::Display for BlockoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "row {}, column {}: {}",
            self.row, self.column, self.reason
        )
    }
}
impl std::error::Error for BlockoutError {}

/// Visit nonempty ASCII cells, row-major, with rows along +Z and columns along
/// +X. Origin is the first cell's center; `.` and space are empty. Accepts LF or
/// CRLF and an optional final newline; does not trim indentation or blank rows.
/// Validates all rows before calling `visit`, so errors have no partial effects.
/// Empty input succeeds. No allocation; invalid cell/origin dimensions panic.
/// ```
/// use exact_game::{place, Vec3};
/// let mut cells = Vec::new();
/// place::blockout("A.\n B", 2.0, Vec3::ZERO, |ch, at| cells.push((ch, at))).unwrap();
/// assert_eq!(cells, [('A', Vec3::ZERO), ('B', Vec3::new(2.0, 0.0, 2.0))]);
/// let e = place::blockout("AA\nA", 1.0, Vec3::ZERO, |_, _| {}).unwrap_err();
/// assert_eq!((e.row, e.column), (2, 2));
/// ```
pub fn blockout(
    map: &str,
    cell: f32,
    origin: Vec3,
    mut visit: impl FnMut(char, Vec3),
) -> Result<(), BlockoutError> {
    nonnegative(cell);
    assert!(origin.is_finite());
    let width = map.lines().next().map_or(0, str::len);
    for (row, line) in map.lines().enumerate() {
        if let Some(column) = line.bytes().position(|b| !b.is_ascii()) {
            return Err(BlockoutError {
                row: row + 1,
                column: column + 1,
                reason: "expected ASCII",
            });
        }
        if line.len() != width {
            return Err(BlockoutError {
                row: row + 1,
                column: line.len().min(width) + 1,
                reason: "ragged row",
            });
        }
    }
    for (row, line) in map.lines().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != '.' && ch != ' ' {
                visit(
                    ch,
                    origin + Vec3::new(col as f32 * cell, 0.0, row as f32 * cell),
                );
            }
        }
    }
    Ok(())
}
