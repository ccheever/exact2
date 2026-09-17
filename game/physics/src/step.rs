use crate::{
    geometry::{collide, world_pose, Geometry, SKIN},
    solver::{self, Constraint, Node},
    Announce, Body, BodyKind, BodyState, Collider, ColliderState, Contact, Manifold, Physics,
    Touch,
};
use exact_game::{Entity, Parent, Transform, Vec2, Vec3, World};
use glam::Mat3;

fn key(m: &Manifold) -> (Entity, Entity) {
    (m.a, m.b)
}
fn index(nodes: &[Node], e: Entity) -> Option<usize> {
    nodes.binary_search_by_key(&e, |n| n.entity).ok()
}
fn passive(n: &Node) -> bool {
    n.body.kind == BodyKind::Static || (n.dynamic() && n.body.asleep)
}
fn collect(world: &World, old: &[ColliderState], nodes: &mut Vec<Node>) {
    nodes.clear();
    for (e, (collider, body, transform, parent)) in world
        .query::<(
            Option<&Collider>,
            Option<&Body>,
            Option<&Transform>,
            Option<&Parent>,
        )>()
        .iter()
    {
        let collider = collider.cloned();
        let body = body.cloned();
        if collider.is_none() && body.is_none() {
            continue;
        }
        if body.is_some() {
            debug_assert!(
                parent.is_none(),
                "physics: Body `{}` must be a root",
                world.name(e).unwrap_or("unnamed")
            );
        }
        let has_body = body.is_some();
        let mut body = body.unwrap_or(Body {
            kind: BodyKind::Static,
            ..Body::default()
        });
        let target = if parent.is_some() {
            world_pose(world, e)
        } else {
            transform.copied().unwrap_or_default()
        };
        let mut pose = target;
        assert!(
            body.velocity.is_finite()
                && body.spin.is_finite()
                && body.mass.is_finite()
                && body.mass >= 0.0
                && body.damping >= 0.0
                && body.spin_damping >= 0.0
                && body.gravity.is_finite(),
            "physics: invalid Body {}",
            world.name(e).unwrap_or("unnamed")
        );
        let mut changed = body.previous.is_some_and(|p| {
            p.pose != target || p.velocity != body.velocity || p.spin != body.spin
        });
        if let Some(c) = &collider {
            assert!(
                c.friction.is_finite() && c.friction >= 0.0 && (0.0..=1.0).contains(&c.bounce),
                "physics: invalid material"
            );
            changed |= old
                .binary_search_by_key(&e, |s| s.entity)
                .ok()
                .is_none_or(|i| old[i].pose != target || old[i].collider != *c);
        }
        let mut inertia = Vec3::ZERO;
        let mut inv_mass = 0.0;
        if body.kind == BodyKind::Dynamic {
            if changed {
                body.asleep = false;
                body.calm = 0;
            }
            (inv_mass, inertia) = collider
                .as_ref()
                .map(|c| Geometry::new(&c.shape, pose).mass(body.mass))
                .unwrap_or((1.0 / body.mass.max(1.0), Vec3::ZERO));
        } else if body.kind == BodyKind::Kinematic {
            if let Some(prev) = body.previous {
                pose = prev.pose;
                body.velocity = (target.position - pose.position) / world.dt();
                let mut q = target.rotation * pose.rotation.conjugate();
                if q.w < 0.0 {
                    q = -q;
                }
                let xyz = Vec3::new(q.x, q.y, q.z);
                let length = xyz.length();
                body.spin = if length > 1e-8 {
                    xyz * (2.0 * exact_game::math::atan2(length, q.w) / (length * world.dt()))
                } else {
                    Vec3::ZERO
                };
            } else {
                body.velocity = Vec3::ZERO;
                body.spin = Vec3::ZERO;
            }
        } else {
            body.velocity = Vec3::ZERO;
            body.spin = Vec3::ZERO;
        }
        let rot = Mat3::from_quat(pose.rotation);
        nodes.push(Node {
            entity: e,
            pose,
            start: pose,
            body,
            has_body,
            collider,
            inv_mass,
            inertia,
            tensor: rot * Mat3::from_diagonal(inertia) * rot.transpose(),
            changed,
        });
    }
}
struct Islands(Vec<usize>);
impl Islands {
    fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    fn root(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let a = self.root(a);
        let b = self.root(b);
        self.0[a.max(b)] = a.min(b);
    }
}
fn wake(nodes: &mut [Node], old: &[Manifold]) {
    let mut islands = Islands::new(nodes.len());
    let mut disturbed = vec![false; nodes.len()];
    for m in old.iter().filter(|m| m.touching && !m.sensor) {
        let a = index(nodes, m.a);
        let b = index(nodes, m.b);
        match (a, b) {
            (Some(a), Some(b)) => {
                if nodes[a].dynamic() && nodes[b].dynamic() {
                    islands.join(a, b);
                }
                if nodes[a].changed {
                    disturbed[b] = true;
                }
                if nodes[b].changed {
                    disturbed[a] = true;
                }
            }
            (Some(a), None) => disturbed[a] = true,
            (None, Some(b)) => disturbed[b] = true,
            _ => {}
        }
    }
    let mut wake = vec![false; nodes.len()];
    for i in 0..nodes.len() {
        if nodes[i].dynamic() && (disturbed[i] || nodes[i].changed) {
            wake[islands.root(i)] = true;
        }
    }
    for (i, n) in nodes.iter_mut().enumerate() {
        if n.dynamic() && wake[islands.root(i)] {
            n.body.asleep = false;
            n.body.calm = 0;
        }
    }
}
fn broadphase(
    nodes: &[Node],
    dt: f32,
    bounds: &mut Vec<(usize, Vec3, Vec3)>,
    statics: &mut Vec<(usize, Vec3, Vec3)>,
    pairs: &mut Vec<(usize, usize)>,
) {
    let count = nodes.iter().filter(|n| n.collider.is_some()).count();
    if bounds.len() + statics.len() != count
        || bounds.iter().any(|b| {
            nodes
                .get(b.0)
                .is_none_or(|n| n.collider.is_none() || n.body.kind == BodyKind::Static)
        })
        || statics.iter().any(|b| {
            nodes
                .get(b.0)
                .is_none_or(|n| n.collider.is_none() || n.body.kind != BodyKind::Static)
        })
    {
        bounds.clear();
        statics.clear();
        for (i, n) in nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.collider.is_some())
        {
            let list = if n.body.kind == BodyKind::Static {
                &mut *statics
            } else {
                &mut *bounds
            };
            list.push((i, Vec3::ZERO, Vec3::ZERO));
        }
    }
    for (i, lo, hi) in bounds.iter_mut().chain(statics.iter_mut()) {
        let n = &nodes[*i];
        let g = Geometry::new(&n.collider.as_ref().unwrap().shape, n.pose);
        let (l, h) = g.aabb();
        let motion = n.body.velocity * dt;
        let pad = Vec3::splat(0.02 + n.body.spin.length() * g.radius() * dt);
        *lo = l + motion.min(Vec3::ZERO) - pad;
        *hi = h + motion.max(Vec3::ZERO) + pad;
    }
    // Coherent X order, with a bounded insertion-sort budget for teleports/spawns.
    // Pair sorting below makes the choice of sorting algorithm unobservable.
    let cmp = |a: &(usize, Vec3, Vec3), b: &(usize, Vec3, Vec3)| {
        a.1.x.total_cmp(&b.1.x).then(a.0.cmp(&b.0))
    };
    let mut swaps = 0;
    'sort: for i in 1..bounds.len() {
        let mut j = i;
        while j > 0 && cmp(&bounds[j], &bounds[j - 1]).is_lt() {
            bounds.swap(j, j - 1);
            j -= 1;
            swaps += 1;
            if swaps > bounds.len() * 8 {
                bounds.sort_unstable_by(cmp);
                break 'sort;
            }
        }
    }
    pairs.clear();
    let mut add = |a: usize, lo: Vec3, hi: Vec3, b: usize, bl: Vec3, bh: Vec3| {
        if !(passive(&nodes[a]) && passive(&nodes[b])) && lo.cmple(bh).all() && bl.cmple(hi).all() {
            let ca = nodes[a].collider.as_ref().unwrap();
            let cb = nodes[b].collider.as_ref().unwrap();
            if ca.layer & cb.mask != 0 && cb.layer & ca.mask != 0 {
                pairs.push((a.min(b), a.max(b)));
            }
        }
    };
    for (i, &(a, lo, hi)) in bounds.iter().enumerate() {
        for &(b, bl, bh) in &bounds[i + 1..] {
            if bl.x > hi.x {
                break;
            }
            add(a, lo, hi, b, bl, bh);
        }
        for &(b, bl, bh) in statics.iter() {
            add(a, lo, hi, b, bl, bh);
        }
    }
    pairs.sort_unstable();
}

fn contacts(
    nodes: &[Node],
    old: &[Manifold],
    dt: f32,
    pairs: &[(usize, usize)],
    observe: &mut impl FnMut(&'static str, usize),
) -> Vec<Manifold> {
    let mut out = Vec::new();
    // Sleeping pairs retain impulses and touching state without rerunning geometry.
    for m in old {
        if let (Some(a), Some(b)) = (index(nodes, m.a), index(nodes, m.b)) {
            if passive(&nodes[a])
                && passive(&nodes[b])
                && !nodes[a].changed
                && !nodes[b].changed
                && nodes[a].collider.is_some()
                && nodes[b].collider.is_some()
            {
                out.push(m.clone());
            }
        }
    }
    observe("matching", 0);
    for &(a, b) in pairs {
        let na = &nodes[a];
        let nb = &nodes[b];
        let ca = na.collider.as_ref().unwrap();
        let cb = nb.collider.as_ref().unwrap();
        let ga = Geometry::new(&ca.shape, na.pose);
        let gb = Geometry::new(&cb.shape, nb.pose);
        let margin = 0.02
            + (nb.body.velocity - na.body.velocity).length() * dt
            + (na.body.spin.length() * ga.radius() + nb.body.spin.length() * gb.radius()) * dt;
        let patch = match (ga, gb) {
            (Geometry::Box { .. }, Geometry::Box { .. }) => {
                crate::boxes::box_box_with_margin(ga, gb, margin)
            }
            _ => Some(collide(ga, gb)),
        };
        observe("narrowphase", 1);
        let Some(patch) = patch else {
            continue;
        };
        if patch.separation() > margin {
            continue;
        }
        let previous = old
            .binary_search_by_key(&(na.entity, nb.entity), key)
            .ok()
            .map(|i| &old[i]);
        let mut points = Vec::new();
        for p in patch.points.into_iter().filter(|p| p.separation <= margin) {
            let prior = previous
                .filter(|m| m.normal.dot(patch.normal) > 0.95)
                .and_then(|m| m.points.iter().find(|c| c.feature == p.feature));
            points.push(Contact {
                feature: p.feature,
                local_a: na.pose.rotation.conjugate() * (p.a - na.pose.position),
                local_b: nb.pose.rotation.conjugate() * (p.b - nb.pose.position),
                separation: p.separation,
                normal_impulse: prior.map_or(0.0, |p| p.normal_impulse),
                tangent_impulse: prior.map_or(Vec2::ZERO, |p| p.tangent_impulse),
            });
        }
        points.sort_by_key(|p| p.feature);
        let touching = points.iter().any(|p| p.separation <= SKIN);
        out.push(Manifold {
            a: na.entity,
            b: nb.entity,
            normal: patch.normal,
            points,
            sensor: ca.sensor || cb.sensor,
            touching,
        });
        observe("matching", 0);
    }
    out.sort_by_key(key);
    observe("matching", 0);
    out
}
fn islands(nodes: &mut [Node], manifolds: &[Manifold], sleep: bool) {
    let mut islands = Islands::new(nodes.len());
    let mut moving_support = vec![false; nodes.len()];
    for m in manifolds.iter().filter(|m| m.touching && !m.sensor) {
        let a = index(nodes, m.a).unwrap();
        let b = index(nodes, m.b).unwrap();
        if nodes[a].dynamic() && nodes[b].dynamic() {
            islands.join(a, b);
        }
        for (x, y) in [(a, b), (b, a)] {
            if nodes[y].body.kind == BodyKind::Kinematic
                && (nodes[y].body.velocity != Vec3::ZERO || nodes[y].body.spin != Vec3::ZERO)
            {
                moving_support[x] = true;
            }
        }
    }
    let mut calm = vec![u32::MAX; nodes.len()];
    let mut awake = vec![false; nodes.len()];
    for (i, n) in nodes.iter_mut().enumerate() {
        if !n.dynamic() {
            continue;
        }
        let root = islands.root(i);
        if sleep {
            if n.body.velocity.length_squared() < 0.0025
                && n.body.spin.length_squared() < 0.0025
                && !moving_support[i]
            {
                n.body.calm = n.body.calm.saturating_add(1);
            } else {
                n.body.calm = 0;
            }
            calm[root] = calm[root].min(n.body.calm);
        } else {
            awake[root] |= !n.body.asleep || moving_support[i];
        }
    }
    for (i, n) in nodes.iter_mut().enumerate() {
        if !n.dynamic() {
            continue;
        }
        let root = islands.root(i);
        if sleep && calm[root] >= 30 {
            n.body.asleep = true;
            n.body.velocity = Vec3::ZERO;
            n.body.spin = Vec3::ZERO;
        } else if !sleep && awake[root] && n.body.asleep {
            n.body.asleep = false;
            n.body.calm = 0;
        }
    }
}
fn transitions(world: &World, old: &[Manifold], new: &[Manifold]) -> Vec<Touch> {
    let mut events = Vec::new();
    for (from, to, began) in [(old, new, false), (new, old, true)] {
        for m in from.iter().filter(|m| m.touching) {
            if to
                .binary_search_by_key(&key(m), key)
                .ok()
                .is_none_or(|i| !to[i].touching)
            {
                events.push(Touch {
                    a: m.a,
                    b: m.b,
                    began,
                });
            }
        }
    }
    events.sort_by_key(|e| (e.a, e.b, e.began));
    for e in &events {
        if world.has::<Announce>(e.a) || world.has::<Announce>(e.b) {
            let a = world
                .name(e.a)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("#{}", e.a.index()));
            let b = world
                .name(e.b)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("#{}", e.b.index()));
            world.log(format!(
                "{} {a} × {b}",
                if e.began { "touch" } else { "untouch" }
            ));
        }
    }
    events
}

// A sleeping world needs only an ordered comparison, not a clone of thousands
// of manifolds. The comparison still observes same-tick writes and despawns.
fn unchanged_asleep(world: &World, physics: &Physics) -> bool {
    for (_, (b, t, parent)) in world
        .query::<(&Body, Option<&Transform>, Option<&Parent>)>()
        .iter()
    {
        if parent.is_some() || (b.kind == BodyKind::Dynamic && !b.asleep) {
            return false;
        }
        let Some(p) = b.previous else {
            return false;
        };
        if p.pose != t.copied().unwrap_or_default() || p.velocity != b.velocity || p.spin != b.spin
        {
            return false;
        }
        if b.kind == BodyKind::Kinematic && (b.velocity != Vec3::ZERO || b.spin != Vec3::ZERO) {
            return false;
        }
    }
    let mut previous = physics.previous.iter();
    for (e, (c, t, parent)) in world
        .query::<(&Collider, Option<&Transform>, Option<&Parent>)>()
        .iter()
    {
        let Some(p) = previous.next() else {
            return false;
        };
        let pose = if parent.is_some() {
            world_pose(world, e)
        } else {
            t.copied().unwrap_or_default()
        };
        if p.entity != e || p.pose != pose || p.collider != *c {
            return false;
        }
    }
    previous.next().is_none()
}
/// Advance physics by exactly `world.dt()`. Call once, after game controls, per tick.
/// Panics on invalid geometry/materials; debug builds name parented Bodies.
/// Reports `world.busy("physics")` until every dynamic body is asleep.
pub fn step(world: &mut World) {
    step_observed(world, |_, _| {});
}

/// Diagnostic phase boundaries; the observer owns its clock outside world state.
/// Each callback ends the named phase; its value is a count or substep index.
#[doc(hidden)]
pub fn step_observed(world: &mut World, mut observe: impl FnMut(&'static str, usize)) {
    {
        let physics = world.resource::<Physics>();
        if unchanged_asleep(world, &physics) {
            drop(physics);
            world.resource_mut::<Physics>().events.clear();
            return;
        }
    }
    let mut physics = std::mem::take(&mut *world.resource_mut::<Physics>());
    let mut scratch = std::mem::take(&mut physics.scratch);
    let nodes = &mut scratch.nodes;
    assert!(
        physics.substeps > 0 && physics.gravity.is_finite(),
        "physics: invalid step configuration"
    );
    collect(world, &physics.previous, nodes);
    observe("gather", 0);
    wake(nodes, &physics.manifolds);
    observe("islands", 0);
    // This still checks all world-owned poses: no hidden cache can miss a game write.
    if nodes.iter().all(|n| passive(n) && !n.changed)
        && physics.previous.len() == nodes.iter().filter(|n| n.collider.is_some()).count()
    {
        physics.events.clear();
        physics.scratch = scratch;
        *world.resource_mut::<Physics>() = physics;
        return;
    }
    broadphase(
        nodes,
        world.dt(),
        &mut scratch.bounds,
        &mut scratch.statics,
        &mut scratch.pairs,
    );
    observe("broadphase", scratch.pairs.len());
    let mut manifolds = contacts(
        nodes,
        &physics.manifolds,
        world.dt(),
        &scratch.pairs,
        &mut observe,
    );
    observe("touching", manifolds.iter().filter(|m| m.touching).count());
    observe("points", manifolds.iter().map(|m| m.points.len()).sum());
    islands(nodes, &manifolds, false);
    let constraints = &mut scratch.constraints;
    constraints.clear();
    let mut dormant = Vec::new();
    for m in manifolds {
        let a = index(nodes, m.a).unwrap();
        let b = index(nodes, m.b).unwrap();
        if m.sensor || (!nodes[a].active() && !nodes[b].active()) {
            dormant.push(m);
        } else {
            constraints.push(Constraint::new(m, a, b, nodes));
        }
    }
    observe("islands", 0);
    solver::solve(
        nodes,
        constraints,
        physics.gravity,
        world.dt(),
        physics.substeps,
        &mut observe,
    );
    manifolds = dormant;
    manifolds.extend(constraints.drain(..).map(|c| c.manifold));
    manifolds.sort_by_key(key);
    for m in &mut manifolds {
        let a = &nodes[index(nodes, m.a).unwrap()];
        let b = &nodes[index(nodes, m.b).unwrap()];
        for p in &mut m.points {
            p.separation = (b.pose.position + b.pose.rotation * p.local_b
                - a.pose.position
                - a.pose.rotation * p.local_a)
                .dot(m.normal);
        }
        m.touching = m.points.iter().any(|p| p.separation <= SKIN);
    }
    islands(nodes, &manifolds, true);
    observe("sleep", 0);
    physics.events = transitions(world, &physics.manifolds, &manifolds);
    physics.manifolds = manifolds;
    physics.previous.clear();
    let mut busy = false;
    for mut n in nodes.drain(..) {
        if n.has_body {
            if n.dynamic() {
                world.insert(n.entity, n.pose);
                busy |= !n.body.asleep;
            } else {
                n.pose = world_pose(world, n.entity);
            }
            n.body.previous = Some(BodyState {
                pose: n.pose,
                velocity: n.body.velocity,
                spin: n.body.spin,
            });
            world.insert(n.entity, n.body);
        }
        if let Some(collider) = n.collider {
            physics.previous.push(ColliderState {
                entity: n.entity,
                pose: n.pose,
                collider,
            });
        }
    }
    physics.scratch = scratch;
    *world.resource_mut::<Physics>() = physics;
    observe("writeback", 0);
    if busy {
        world.busy("physics");
    }
}
