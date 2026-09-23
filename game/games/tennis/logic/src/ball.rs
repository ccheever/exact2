//! The ball and the court it flies over: gravity, quadratic drag and Magnus
//! lift in flight; restitution and Coulomb friction at the bounce; the net as a
//! plane at z = 0 whose tape sags from post to centre. Every function here is
//! pure and uses the tick's own integrator, so a prediction is the future.
use exact_game::{math, Data, Vec2, Vec3};

/// Singles court, metres, measured to the outside of the lines (ITF).
pub const HALF_LENGTH: f32 = 11.885;
pub const HALF_WIDTH: f32 = 4.115;
pub const SERVICE_LINE: f32 = 6.40;
pub const NET_CENTER: f32 = 0.914;
pub const NET_POST: f32 = 1.07;
/// Singles sticks stand 0.914 m outside each sideline.
pub const POST_X: f32 = 5.029;
pub const RADIUS: f32 = 0.0335;

const GRAVITY: f32 = 9.81;
/// ½ ρ C_d A / m for a 57 g ball, C_d 0.55, per metre.
const DRAG: f32 = 0.0205;
/// Magnus acceleration per (rad/s × m/s); ~0.45 g for a 2,000 rpm drive at 30 m/s.
const MAGNUS: f32 = 0.00075;
const SPIN_DECAY: f32 = 0.03;
/// Vertical coefficient of restitution on a hard court.
const RESTITUTION: f32 = 0.76;
const FRICTION: f32 = 0.55;
const SUBSTEPS: u32 = 2;
pub const RPM: f32 = std::f32::consts::TAU / 60.0;

/// Where the ball is, how it moves and how it spins (rad/s, world axes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Data)]
pub struct Flight {
    pub pos: Vec3,
    pub vel: Vec3,
    pub spin: Vec3,
}

/// What the ball touched during one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Air,
    Bounce(Vec3),
    Net,
    Cord,
}

pub fn net_height(x: f32) -> f32 {
    NET_CENTER + (NET_POST - NET_CENTER) * (x.abs() / POST_X).min(1.0)
}

impl Flight {
    /// Advance one fixed tick; reports the first thing touched.
    pub fn step(&mut self, dt: f32) -> Touch {
        let mut touch = Touch::Air;
        for _ in 0..SUBSTEPS {
            let now = self.substep(dt / SUBSTEPS as f32);
            if touch == Touch::Air {
                touch = now;
            }
        }
        touch
    }

    fn substep(&mut self, h: f32) -> Touch {
        let speed = self.vel.length();
        let accel = Vec3::new(0.0, -GRAVITY, 0.0) - self.vel * (DRAG * speed)
            + self.spin.cross(self.vel) * MAGNUS;
        self.vel += accel * h;
        let before = self.pos;
        self.pos += self.vel * h;
        self.spin *= 1.0 - SPIN_DECAY * h;
        if (before.z > 0.0) != (self.pos.z > 0.0) {
            let t = before.z / (before.z - self.pos.z);
            let at = before + (self.pos - before) * t;
            let top = net_height(at.x);
            if at.x.abs() <= POST_X && at.y < top - RADIUS * 0.5 {
                // Into the mesh: it drops dead on the hitter's side.
                let back = if before.z > 0.0 { 1.0 } else { -1.0 };
                self.pos = Vec3::new(at.x, at.y, back * (RADIUS + 0.01));
                self.vel = Vec3::new(
                    self.vel.x * 0.3,
                    self.vel.y.min(0.0) * 0.2,
                    -self.vel.z * 0.12,
                );
                self.spin *= 0.2;
                return Touch::Net;
            }
            if at.x.abs() <= POST_X && at.y < top + RADIUS {
                // Clips the tape and dribbles on, slower and higher.
                self.vel = Vec3::new(
                    self.vel.x * 0.8,
                    self.vel.y.abs() * 0.25 + 0.8,
                    self.vel.z * 0.5,
                );
                self.spin *= 0.3;
                return Touch::Cord;
            }
        }
        if self.pos.y < RADIUS && self.vel.y < 0.0 {
            self.pos.y = RADIUS;
            if self.vel.y > -0.6 {
                self.vel.y = 0.0;
                return Touch::Air;
            }
            self.bounce();
            return Touch::Bounce(self.pos);
        }
        if self.vel.y == 0.0 && self.pos.y <= RADIUS {
            let roll = 1.0 - 1.5 * h;
            self.vel.x *= roll;
            self.vel.z *= roll;
        }
        Touch::Air
    }

    /// Normal restitution, then a friction impulse capped at rolling
    /// (a hollow sphere: slip changes by 2.5 × the impulse per unit mass).
    /// Topspin arrives with less slip, keeps its pace and kicks; slice arrives
    /// flat and skids.
    fn bounce(&mut self) {
        let vy = self.vel.y;
        self.vel.y = -RESTITUTION * vy;
        let slip = Vec3::new(
            self.vel.x + RADIUS * self.spin.z,
            0.0,
            self.vel.z - RADIUS * self.spin.x,
        );
        let s = slip.length();
        if s > 1e-6 {
            let j = (FRICTION * (1.0 + RESTITUTION) * -vy).min(s / 2.5);
            let f = slip * (-j / s);
            self.vel.x += f.x;
            self.vel.z += f.z;
            self.spin.x -= 1.5 / RADIUS * f.z;
            self.spin.z += 1.5 / RADIUS * f.x;
        }
        self.spin.y *= 0.7;
    }
}

/// Where the first bounce lands, or None if the net stops it within `seconds`.
pub fn landing(flight: Flight, dt: f32, seconds: f32) -> Option<Vec3> {
    first_bounce(flight, dt, seconds, false)
}

fn first_bounce(mut flight: Flight, dt: f32, seconds: f32, clean: bool) -> Option<Vec3> {
    for _ in 0..(seconds / dt) as u32 {
        match flight.step(dt) {
            Touch::Bounce(at) => return Some(at),
            Touch::Net => return None,
            Touch::Cord if clean => return None,
            _ => {}
        }
    }
    None
}

/// A stroke's pace and spin: rpm is topspin when positive, backspin when
/// negative; side is sidespin rpm (positive curves toward +x either way).
/// Lobs search the high arc.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stroke {
    pub speed: f32,
    pub rpm: f32,
    pub side: f32,
    pub lob: bool,
}

/// Launch from `from` so the first bounce lands on `target` (x, z): bisect the
/// elevation for depth, then correct the aim for curl. A target the pace cannot
/// reach lands where the nearest achievable arc does, net clearance first.
pub fn launch(from: Vec3, target: Vec2, stroke: Stroke, dt: f32) -> Flight {
    let mut aim = target;
    let mut flight = aimed(from, aim, target, stroke, dt);
    for _ in 0..3 {
        let Some(landed) = landing(flight, dt, 4.0) else {
            break;
        };
        let miss = Vec2::new(landed.x - target.x, landed.z - target.y);
        let dir = (target - Vec2::new(from.x, from.z)).normalize_or_zero();
        let lateral = miss - dir * miss.dot(dir);
        if lateral.length() < 0.02 {
            break;
        }
        aim -= lateral;
        flight = aimed(from, aim, target, stroke, dt);
    }
    flight
}

fn aimed(from: Vec3, aim: Vec2, target: Vec2, stroke: Stroke, dt: f32) -> Flight {
    let origin = Vec2::new(from.x, from.z);
    let flat = (aim - origin).normalize_or_zero();
    let dir = Vec3::new(flat.x, 0.0, flat.y);
    let want = (target - origin).length();
    let spin =
        Vec3::Y.cross(dir) * (stroke.rpm * RPM) + Vec3::Y * (stroke.side * RPM * dir.z.signum());
    let make = |elevation: f32| Flight {
        pos: from,
        vel: (dir * math::cos(elevation) + Vec3::Y * math::sin(elevation)) * stroke.speed,
        spin,
    };
    let (mut low, mut high) = if stroke.lob {
        (0.72, 1.3)
    } else {
        (-0.35, 0.72)
    };
    for _ in 0..22 {
        let mid = 0.5 * (low + high);
        // A ball that touches the tape counts as short: aim clears the net.
        let reach = first_bounce(make(mid), dt, 4.0, true)
            .map_or(0.0, |p| (Vec2::new(p.x, p.z) - origin).dot(flat));
        // Low arcs reach further as they rise; lobs reach less.
        if (reach < want) != stroke.lob {
            low = mid;
        } else {
            high = mid;
        }
    }
    // Keep the bound that reached the target: it is the one that clears the net.
    make(if stroke.lob { low } else { high })
}

/// A uniform point in a disc of `radius`, from two unit draws.
pub fn scatter(radius: f32, u: f32, v: f32) -> Vec2 {
    let r = radius * math::sqrt(u);
    let a = std::f32::consts::TAU * v;
    Vec2::new(r * math::cos(a), r * math::sin(a))
}

/// Is a bounce at `at` inside the singles court (lines are in)?
pub fn in_court(at: Vec3) -> bool {
    at.x.abs() <= HALF_WIDTH + 0.01 && at.z.abs() <= HALF_LENGTH + 0.01
}

/// Is a bounce at `at` inside the service box on the half with sign `half`
/// (+1 near, -1 far), on the receiver's `deuce` side?
pub fn in_box(at: Vec3, half: f32, deuce: bool) -> bool {
    // The receiver's right is +x on the near half (facing -z), -x on the far.
    let right = if deuce { half } else { -half };
    at.z * half >= -0.01
        && at.z * half <= SERVICE_LINE + 0.01
        && at.x * right >= -0.01
        && at.x * right <= HALF_WIDTH + 0.01
}

#[cfg(test)]
mod tests {
    use super::*;
    const DT: f32 = 1.0 / 120.0;

    #[test]
    fn topspin_dips_and_kicks_slice_floats_and_skids() {
        let from = Vec3::new(0.0, 1.0, 11.0);
        let flat = Stroke {
            speed: 26.0,
            rpm: 0.0,
            side: 0.0,
            lob: false,
        };
        let top = Stroke {
            rpm: 2600.0,
            ..flat
        };
        let slice = Stroke {
            rpm: -1800.0,
            ..flat
        };
        let target = Vec2::new(0.0, -9.0);
        let bounce = |s: Stroke| {
            let mut f = launch(from, target, s, DT);
            let mut max_before = 0.0f32;
            loop {
                if let Touch::Bounce(at) = f.step(DT) {
                    return (at, f, max_before);
                }
                max_before = max_before.max(f.pos.y);
            }
        };
        let (a_top, f_top, apex_top) = bounce(top);
        let (a_slice, f_slice, apex_slice) = bounce(slice);
        assert!((a_top.z - target.y).abs() < 0.05 && (a_slice.z - target.y).abs() < 0.05);
        assert!(
            apex_top > apex_slice,
            "topspin needs a higher arc to land the same spot"
        );
        assert!(f_top.vel.y > f_slice.vel.y, "topspin kicks up");
        assert!(
            -f_slice.vel.z > 0.0 && f_top.vel.y / -f_top.vel.z > f_slice.vel.y / -f_slice.vel.z,
            "slice skids lower"
        );
    }

    #[test]
    fn nets_and_boxes() {
        let mut low = Flight {
            pos: Vec3::new(0.0, 0.5, 2.0),
            vel: Vec3::new(0.0, 0.0, -20.0),
            spin: Vec3::ZERO,
        };
        assert!((0..60).any(|_| low.step(DT) == Touch::Net));
        assert!(low.pos.z > 0.0);
        // The far receiver's right, their deuce court, is -x.
        assert!(in_box(Vec3::new(-2.0, 0.0, -5.0), -1.0, true));
        assert!(!in_box(Vec3::new(-2.0, 0.0, -5.0), -1.0, false));
        assert!(in_box(Vec3::new(2.0, 0.0, 5.0), 1.0, true));
        assert!(!in_box(Vec3::new(2.0, 0.0, 7.0), 1.0, true));
    }
}
