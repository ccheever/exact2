use crate::{
    geometry::{basis, integrate},
    Body, BodyKind, Collider, Manifold,
};
use exact_game::{Entity, Transform, Vec2, Vec3};
use glam::Mat3;

pub(crate) struct Node {
    pub entity: Entity,
    pub pose: Transform,
    pub start: Transform,
    pub body: Body,
    pub has_body: bool,
    pub collider: Option<Collider>,
    pub inv_mass: f32,
    pub inertia: Vec3,
    pub tensor: Mat3,
    pub changed: bool,
}
impl Node {
    pub fn dynamic(&self) -> bool {
        self.body.kind == BodyKind::Dynamic
    }
    pub fn active(&self) -> bool {
        self.dynamic() && !self.body.asleep
    }
    pub fn velocity(&self, r: Vec3) -> Vec3 {
        self.body.velocity + self.body.spin.cross(r)
    }
    pub fn impulse(&mut self, p: Vec3, r: Vec3) {
        if self.active() {
            self.body.velocity += p * self.inv_mass;
            self.body.spin += self.tensor * r.cross(p);
        }
    }
    fn update_tensor(&mut self) {
        let rot = Mat3::from_quat(self.pose.rotation);
        self.tensor = rot * Mat3::from_diagonal(self.inertia) * rot.transpose();
    }
}
#[derive(Default)]
struct Prepared {
    ra: Vec3,
    rb: Vec3,
    mass: f32,
    tangent: Vec2,
    cross: f32,
    approach: f32,
    maximum: f32,
}
pub(crate) struct Constraint {
    pub manifold: Manifold,
    pub a: usize,
    pub b: usize,
    friction: f32,
    bounce: f32,
    tangent: [Vec3; 2],
    points: Vec<Prepared>,
}
fn two(nodes: &mut [Node], a: usize, b: usize) -> (&mut Node, &mut Node) {
    let (left, right) = nodes.split_at_mut(b);
    (&mut left[a], &mut right[0])
}
impl Constraint {
    pub fn new(manifold: Manifold, a: usize, b: usize, nodes: &[Node]) -> Self {
        let ca = nodes[a].collider.as_ref().unwrap();
        let cb = nodes[b].collider.as_ref().unwrap();
        let (t, u) = basis(manifold.normal);
        let points = manifold
            .points
            .iter()
            .map(|p| {
                let ra = nodes[a].pose.rotation * p.local_a;
                let rb = nodes[b].pose.rotation * p.local_b;
                Prepared {
                    approach: (nodes[b].velocity(rb) - nodes[a].velocity(ra)).dot(manifold.normal),
                    ..Prepared::default()
                }
            })
            .collect();
        Self {
            manifold,
            a,
            b,
            friction: (ca.friction * cb.friction).sqrt(),
            bounce: ca.bounce.max(cb.bounce),
            tangent: [t, u],
            points,
        }
    }
    fn prepare(&mut self, nodes: &[Node]) {
        let a = &nodes[self.a];
        let b = &nodes[self.b];
        let n = self.manifold.normal;
        for (p, c) in self.manifold.points.iter_mut().zip(&mut self.points) {
            c.ra = a.pose.rotation * p.local_a;
            c.rb = b.pose.rotation * p.local_b;
            p.separation = (b.pose.position + c.rb - a.pose.position - c.ra).dot(n);
            c.mass = 1.0 / effective(a, b, c.ra, c.rb, n, n).max(1e-12);
            c.tangent = Vec2::new(
                effective(a, b, c.ra, c.rb, self.tangent[0], self.tangent[0]),
                effective(a, b, c.ra, c.rb, self.tangent[1], self.tangent[1]),
            );
            c.cross = effective(a, b, c.ra, c.rb, self.tangent[0], self.tangent[1]);
        }
    }
    fn warm(&self, nodes: &mut [Node]) {
        let (a, b) = two(nodes, self.a, self.b);
        for (p, c) in self.manifold.points.iter().zip(&self.points) {
            let impulse = self.manifold.normal * p.normal_impulse
                + self.tangent[0] * p.tangent_impulse.x
                + self.tangent[1] * p.tangent_impulse.y;
            a.impulse(-impulse, c.ra);
            b.impulse(impulse, c.rb);
        }
    }
    fn solve(&mut self, nodes: &mut [Node], h: f32, bias: bool) {
        let (a, b) = two(nodes, self.a, self.b);
        let n = self.manifold.normal;
        // Catto's mass-independent soft constraint coefficients: 30 Hz, zeta=10.
        let omega = std::f32::consts::TAU * 30.0;
        let a1 = 20.0 + h * omega;
        let a2 = h * omega * a1;
        let impulse_scale = 1.0 / (1.0 + a2);
        for (p, c) in self.manifold.points.iter_mut().zip(&mut self.points) {
            let separation = (b.pose.position + c.rb - a.pose.position - c.ra).dot(n);
            let vn = (b.velocity(c.rb) - a.velocity(c.ra)).dot(n);
            let (push, mass_scale, soft) = if separation > 0.0 {
                (separation / h, 1.0, 0.0)
            } else if bias {
                (
                    (omega / a1 * (separation + 0.0001).min(0.0)).max(-3.0),
                    a2 * impulse_scale,
                    impulse_scale,
                )
            } else {
                (0.0, 1.0, 0.0)
            };
            let delta = -mass_scale * c.mass * (vn + push) - soft * p.normal_impulse;
            let next = (p.normal_impulse + delta).max(0.0);
            let impulse = n * (next - p.normal_impulse);
            p.normal_impulse = next;
            c.maximum = c.maximum.max(next);
            a.impulse(-impulse, c.ra);
            b.impulse(impulse, c.rb);
            let v = b.velocity(c.rb) - a.velocity(c.ra);
            let vt = Vec2::new(v.dot(self.tangent[0]), v.dot(self.tangent[1]));
            let det = c.tangent.x * c.tangent.y - c.cross * c.cross;
            let delta = if det > 1e-16 {
                -Vec2::new(
                    c.tangent.y * vt.x - c.cross * vt.y,
                    c.tangent.x * vt.y - c.cross * vt.x,
                ) / det
            } else {
                Vec2::ZERO
            };
            let mut next = p.tangent_impulse + delta;
            let limit = self.friction * p.normal_impulse;
            if next.length_squared() > limit * limit {
                next = next * (limit / next.length());
            }
            let d = next - p.tangent_impulse;
            p.tangent_impulse = next;
            let impulse = self.tangent[0] * d.x + self.tangent[1] * d.y;
            a.impulse(-impulse, c.ra);
            b.impulse(impulse, c.rb);
        }
    }
    fn restitution(&mut self, nodes: &mut [Node]) {
        if self.bounce == 0.0 {
            return;
        }
        let (a, b) = two(nodes, self.a, self.b);
        let n = self.manifold.normal;
        for (p, c) in self.manifold.points.iter_mut().zip(&self.points) {
            if c.approach >= -1.0 || c.maximum == 0.0 {
                continue;
            }
            let vn = (b.velocity(c.rb) - a.velocity(c.ra)).dot(n);
            let impulse = (-c.mass * (vn + self.bounce * c.approach)).max(0.0);
            a.impulse(-n * impulse, c.ra);
            b.impulse(n * impulse, c.rb);
            // Restitution is transient for warm-start purposes; do not feed the
            // bounce back into next tick's support force.
            p.normal_impulse = 0.0;
            p.tangent_impulse = Vec2::ZERO;
        }
    }
}
fn effective(a: &Node, b: &Node, ra: Vec3, rb: Vec3, x: Vec3, y: Vec3) -> f32 {
    (a.inv_mass + b.inv_mass) * x.dot(y)
        + ra.cross(x).dot(a.tensor * ra.cross(y))
        + rb.cross(x).dot(b.tensor * rb.cross(y))
}
pub(crate) fn solve(
    nodes: &mut [Node],
    constraints: &mut [Constraint],
    gravity: Vec3,
    dt: f32,
    substeps: u32,
) {
    let h = dt / substeps as f32;
    for sub in 0..substeps {
        for n in nodes.iter_mut() {
            if n.active() {
                n.body.velocity =
                    (n.body.velocity + gravity * (n.body.gravity * h)) / (1.0 + h * n.body.damping);
                n.body.spin /= 1.0 + h * n.body.spin_damping;
                n.update_tensor();
            } else if n.body.kind == BodyKind::Kinematic {
                // The game already placed the kinematic at its end-of-tick pose.
                n.pose.position = n.start.position + n.body.velocity * (h * sub as f32);
            }
        }
        for c in constraints.iter_mut() {
            c.prepare(nodes);
            c.warm(nodes);
        }
        for _ in 0..4 {
            for c in constraints.iter_mut() {
                c.solve(nodes, h, true);
            }
        }
        for n in nodes.iter_mut() {
            if n.active() || n.body.kind == BodyKind::Kinematic {
                n.pose.position += n.body.velocity * h;
                n.pose.rotation = integrate(n.pose.rotation, n.body.spin, h);
            }
        }
        for c in constraints.iter_mut() {
            c.solve(nodes, h, false);
        }
    }
    for c in constraints.iter_mut() {
        c.restitution(nodes);
    }
}
