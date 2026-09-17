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
fn collect(world: &World, old: &[ColliderState]) -> Vec<Node> {
    let mut nodes = Vec::new();
    for e in world.entities() {
        let collider = world.get::<Collider>(e).map(|c| c.clone());
        let body = world.get::<Body>(e).map(|b| b.clone());
        if collider.is_none() && body.is_none() {
            continue;
        }
        if body.is_some() {
            debug_assert!(
                !world.has::<Parent>(e),
                "physics: Body `{}` must be a root",
                world.name(e).unwrap_or("unnamed")
            );
        }
        let has_body = body.is_some();
        let mut body = body.unwrap_or(Body {
            kind: BodyKind::Static,
            ..Body::default()
        });
        let target = world_pose(world, e);
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
    nodes
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
fn broadphase(nodes: &[Node], dt: f32) -> Vec<(usize, usize)> {
    let mut bounds = Vec::new();
    let mut mean = Vec3::ZERO;
    for (i, n) in nodes.iter().enumerate() {
        let Some(c) = &n.collider else {
            continue;
        };
        let g = Geometry::new(&c.shape, n.pose);
        let (lo, hi) = g.aabb();
        let motion = n.body.velocity * dt;
        let pad = Vec3::splat(0.02 + n.body.spin.length() * g.radius() * dt);
        bounds.push((
            i,
            lo + motion.min(Vec3::ZERO) - pad,
            hi + motion.max(Vec3::ZERO) + pad,
        ));
        mean += (lo + hi) * 0.5;
    }
    mean /= bounds.len().max(1) as f32;
    let mut variance = Vec3::ZERO;
    for (_, lo, hi) in &bounds {
        let d = (*lo + *hi) * 0.5 - mean;
        variance += d * d;
    }
    let axis = if variance.x >= variance.y && variance.x >= variance.z {
        0
    } else if variance.y >= variance.z {
        1
    } else {
        2
    };
    bounds.sort_by(|a, b| a.1[axis].total_cmp(&b.1[axis]).then(a.0.cmp(&b.0)));
    let mut pairs = Vec::new();
    for (i, &(a, lo, hi)) in bounds.iter().enumerate() {
        for &(b, bl, bh) in &bounds[i + 1..] {
            if bl[axis] > hi[axis] {
                break;
            }
            if passive(&nodes[a]) && passive(&nodes[b]) {
                continue;
            }
            if lo.cmple(bh).all() && bl.cmple(hi).all() {
                let ca = nodes[a].collider.as_ref().unwrap();
                let cb = nodes[b].collider.as_ref().unwrap();
                if ca.layer & cb.mask != 0 && cb.layer & ca.mask != 0 {
                    pairs.push((a.min(b), a.max(b)));
                }
            }
        }
    }
    pairs.sort_unstable();
    pairs
}
fn contacts(nodes: &[Node], old: &[Manifold], dt: f32) -> Vec<Manifold> {
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
    for (a, b) in broadphase(nodes, dt) {
        let na = &nodes[a];
        let nb = &nodes[b];
        let ca = na.collider.as_ref().unwrap();
        let cb = nb.collider.as_ref().unwrap();
        let ga = Geometry::new(&ca.shape, na.pose);
        let gb = Geometry::new(&cb.shape, nb.pose);
        let patch = collide(ga, gb);
        let margin = 0.02
            + (nb.body.velocity - na.body.velocity).length() * dt
            + (na.body.spin.length() * ga.radius() + nb.body.spin.length() * gb.radius()) * dt;
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
    }
    out.sort_by_key(key);
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
    {
        let physics = world.resource::<Physics>();
        if unchanged_asleep(world, &physics) {
            drop(physics);
            world.resource_mut::<Physics>().events.clear();
            return;
        }
    }
    let mut physics = world.resource::<Physics>().clone();
    assert!(
        physics.substeps > 0 && physics.gravity.is_finite(),
        "physics: invalid step configuration"
    );
    let mut nodes = collect(world, &physics.previous);
    wake(&mut nodes, &physics.manifolds);
    // This still checks all world-owned poses: no hidden cache can miss a game write.
    if nodes.iter().all(|n| passive(n) && !n.changed)
        && physics.previous.len() == nodes.iter().filter(|n| n.collider.is_some()).count()
    {
        world.resource_mut::<Physics>().events.clear();
        return;
    }
    let mut manifolds = contacts(&nodes, &physics.manifolds, world.dt());
    islands(&mut nodes, &manifolds, false);
    let mut constraints = Vec::new();
    let mut dormant = Vec::new();
    for m in manifolds {
        let a = index(&nodes, m.a).unwrap();
        let b = index(&nodes, m.b).unwrap();
        if m.sensor || (!nodes[a].active() && !nodes[b].active()) {
            dormant.push(m);
        } else {
            constraints.push(Constraint::new(m, a, b, &nodes));
        }
    }
    solver::solve(
        &mut nodes,
        &mut constraints,
        physics.gravity,
        world.dt(),
        physics.substeps,
    );
    manifolds = dormant;
    manifolds.extend(constraints.into_iter().map(|c| c.manifold));
    manifolds.sort_by_key(key);
    for m in &mut manifolds {
        let a = &nodes[index(&nodes, m.a).unwrap()];
        let b = &nodes[index(&nodes, m.b).unwrap()];
        for p in &mut m.points {
            p.separation = (b.pose.position + b.pose.rotation * p.local_b
                - a.pose.position
                - a.pose.rotation * p.local_a)
                .dot(m.normal);
        }
        m.touching = m.points.iter().any(|p| p.separation <= SKIN);
    }
    islands(&mut nodes, &manifolds, true);
    physics.events = transitions(world, &physics.manifolds, &manifolds);
    physics.manifolds = manifolds;
    physics.previous.clear();
    let mut busy = false;
    for mut n in nodes {
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
    world.insert_resource(physics);
    if busy {
        world.busy("physics");
    }
}
