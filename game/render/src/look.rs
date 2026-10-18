//! Look frames' requests and answers (LLP 1127.000 D1, D3, D4): the agent's
//! `screenshot … world from …`, the tells at `clock settle` and `screenshot`,
//! `state world draws` and `state world:<entity> visible`. Each is answered
//! from one look frame, the current tick drawn offscreen (`surface/look.rs`),
//! and only under the agent's clock.
use crate::renderer::probe::{Drew, Facts, Reading, Record, Stage};
use crate::tells::{Drawn, FrameStats, Tell};
use exact_game::{Data, Entity, Mesh, Vec2, Vec3, World};
use std::fmt::Write;

/// What a look request asks for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ask {
    /// The frame's pixels, from `view`, with its tells.
    Picture(View),
    /// The tells alone, from the game's camera.
    Tells,
    /// Every draw the frame issued.
    Draws,
    /// Whether one entity drew, and if not, why.
    Visible(String),
}

/// Where a look frame is seen from. Presentation only: the simulation never
/// learns a look frame was drawn, and the world hash does not move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum View {
    /// The game's own camera.
    Camera,
    /// Degrees of yaw about +Y (0 looks along −Z) and of pitch (negative looks
    /// down), `distance` metres from the centre of the drawn scene.
    Orbit { yaw: f32, pitch: f32, distance: f32 },
    /// An orthographic plan from above, `height` metres tall, north (−Z) up.
    Top { height: f32 },
}

#[derive(Data)]
struct Wire {
    op: String,
    form: String,
    look: bool,
    draws: bool,
    visible: bool,
    entity: String,
    yaw: f32,
    pitch: f32,
    distance: f32,
    top: f32,
}
impl Default for Wire {
    fn default() -> Self {
        Self {
            op: String::new(),
            form: String::new(),
            look: false,
            draws: false,
            visible: false,
            entity: String::new(),
            yaw: f32::NAN,
            pitch: f32::NAN,
            distance: f32::NAN,
            top: f32::NAN,
        }
    }
}

/// The look request in an agent request, if it is one.
pub(crate) fn ask(request: &str) -> Result<Option<Ask>, String> {
    let w: Wire = match exact_game::json::from_str(request) {
        Ok(w) => w,
        // Not a look request's shape; the simulation answers it, or refuses it.
        Err(_) => return Ok(None),
    };
    Ok(Some(match (w.op.as_str(), w.form.as_str()) {
        ("screenshot", "look") => Ask::Picture(view(&w)?),
        ("state", _) if w.visible => {
            if w.entity.is_empty() {
                return Err("visible needs an entity: `state world:<name> visible`".into());
            }
            Ask::Visible(w.entity)
        }
        ("state", _) if w.draws => Ask::Draws,
        ("state", _) if w.look => Ask::Tells,
        _ => return Ok(None),
    }))
}

fn view(w: &Wire) -> Result<View, String> {
    let given = |v: f32| !v.is_nan();
    if given(w.top) {
        return (w.top.is_finite() && w.top > 0.)
            .then_some(View::Top { height: w.top })
            .ok_or_else(|| "from top <height>: a positive height in metres".into());
    }
    if [w.yaw, w.pitch, w.distance].iter().all(|v| given(*v)) {
        let ok = [w.yaw, w.pitch, w.distance].iter().all(|v| v.is_finite()) && w.distance > 0.;
        return ok
            .then_some(View::Orbit {
                yaw: w.yaw,
                pitch: w.pitch.clamp(-89.9, 89.9),
                distance: w.distance,
            })
            .ok_or_else(|| {
                "from <yaw> <pitch> <distance>: degrees, degrees, positive metres".into()
            });
    }
    Ok(View::Camera)
}

/// The drawn scene's centre and radius: the box around every drawn entity's
/// origin (meshes, sprites, emitters).
pub(crate) fn scene_bounds(w: &World) -> (Vec3, f32) {
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut grow = |e: Entity| {
        if let Some(drawn) = w.drawn(e) {
            let p = Vec3::from(drawn.pose.translation);
            lo = lo.min(p);
            hi = hi.max(p);
        }
    };
    w.query::<&Mesh>().iter().for_each(|(e, _)| grow(e));
    w.query::<&exact_game::Sprite>()
        .iter()
        .for_each(|(e, _)| grow(e));
    w.query::<&exact_game::Emitter>()
        .iter()
        .for_each(|(e, _)| grow(e));
    if !lo.is_finite() {
        return (Vec3::ZERO, 1.0);
    }
    ((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1.0))
}

impl View {
    /// Point `input` at this view; the game's camera leaves it as it is.
    pub(crate) fn apply(self, input: &mut crate::FrameInput<'_>, w: &World, size: Vec2) {
        let (centre, radius) = scene_bounds(w);
        let (eye, up, camera) = match self {
            View::Camera => return,
            View::Orbit {
                yaw,
                pitch,
                distance,
            } => {
                let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
                let to_eye = Vec3::new(
                    yaw.sin() * pitch.cos(),
                    -pitch.sin(),
                    yaw.cos() * pitch.cos(),
                );
                let far = (distance + radius) * 4.;
                let camera = exact_game::Camera {
                    far,
                    near: (far * 1e-4).max(0.05),
                    ..exact_game::Camera::default()
                };
                (centre + to_eye * distance, Vec3::Y, camera)
            }
            View::Top { height } => {
                let lift = radius * 2. + 10.;
                let camera = exact_game::Camera {
                    far: lift * 2.,
                    ..exact_game::Camera::orthographic(height)
                };
                (centre + Vec3::Y * lift, -Vec3::Z, camera)
            }
        };
        input.view = glam::camera::rh::view::look_at_mat4(eye, centre, up);
        input.proj = camera.matrix(size);
        input.camera_position = eye;
    }
}

/// The tells' inputs that are not pixels: particles drawn and covered, and a
/// shadowed sun's casters after culling.
pub(crate) fn drawn(records: &[Record], facts: &Facts, read: &Reading) -> Drawn {
    let covered = |r: &Record| {
        r.query
            .is_some_and(|q| read.covered.get(q as usize) == Some(&true))
    };
    let particles = (records.iter())
        .filter(|r| {
            matches!(
                r.drew,
                Drew::Quads {
                    kind: "particles",
                    ..
                }
            )
        })
        .fold(None, |acc: Option<(u64, bool)>, r| {
            let Drew::Quads { count, .. } = r.drew else {
                unreachable!()
            };
            let (live, any) = acc.unwrap_or_default();
            Some((live + u64::from(count), any || covered(r)))
        });
    let sun_casters = (facts.cascades > 0).then(|| {
        (facts.groups.iter().enumerate())
            .map(|(g, fact)| {
                (1..=facts.cascades)
                    .filter(|v| fact.views & (1 << v) != 0)
                    .map(|v| kept(facts, read, g, v as usize).unwrap_or(fact.range.len() as u64))
                    .sum::<u64>()
            })
            .sum()
    });
    Drawn {
        particles,
        sun_casters,
    }
}

/// Instances group `g` kept in view `v` after culling, when the frame was culled.
fn kept(facts: &Facts, read: &Reading, g: usize, v: usize) -> Option<u64> {
    let at = (facts.groups[g].offsets[v].1 / 4) as usize + 1;
    (!facts.direct).then(|| read.indirect.get(at).map(|&n| u64::from(n)))?
}

/// The slots group `g` kept in view `v`.
fn list<'a>(facts: &Facts, read: &'a Reading, g: usize, v: usize) -> Option<&'a [u32]> {
    let n = kept(facts, read, g, v)? as usize;
    let at = (facts.groups[g].offsets[v].0 / 4) as usize;
    read.lists.get(at..at + n)
}

/// The tells' JSON: statistics, tells (with whether each is silenced) and cost.
pub(crate) fn tells_json(stats: &FrameStats, drawn: Drawn, tells: &[Tell], bytes: u64) -> String {
    let mut out = format!(
        "{{\"mean\":{},\"clipped\":{},\"histogram\":{:?},\"tells\":[",
        round(stats.mean),
        round(stats.clipped),
        stats.histogram
    );
    for (i, t) in tells.iter().enumerate() {
        let comma = if i == 0 { "" } else { "," };
        let _ = write!(
            out,
            "{comma}{{\"name\":\"{}\",\"line\":{}}}",
            t.name,
            quote(&t.line)
        );
    }
    out.push(']');
    match drawn.particles {
        Some((live, covered)) => {
            let _ = write!(
                out,
                ",\"particles\":{{\"live\":{live},\"covered\":{covered}}}"
            );
        }
        None => out.push_str(",\"particles\":null"),
    }
    match drawn.sun_casters {
        Some(n) => {
            let _ = write!(out, ",\"sunCasters\":{n}");
        }
        None => out.push_str(",\"sunCasters\":null"),
    }
    let _ = write!(out, ",\"readbackBytes\":{bytes}}}");
    out
}

fn round(v: f32) -> f32 {
    (v * 10_000.).round() / 10_000.
}
pub(crate) fn quote(s: &str) -> String {
    exact_game::json::to_string(&s.to_owned()).unwrap()
}

/// Labels for the draw listing: a batch's mesh or model, and its material.
pub(crate) struct Labels {
    pub batches: Vec<String>,
}

/// `state world draws`: each draw in pass order with its batch, what it was
/// submitted and kept after culling, whether it covered a sample, and what it wrote.
pub(crate) fn draws_json(
    records: &[Record],
    facts: &Facts,
    read: &Reading,
    labels: &Labels,
) -> String {
    let mut out = String::from("[");
    for (i, r) in records.iter().enumerate() {
        if i != 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"pass\":{}", quote(&r.stage.name()));
        match r.drew {
            Drew::Group {
                group,
                view,
                variant,
            } => {
                let fact = &facts.groups[group];
                let label = labels.batches.get(fact.batch).map_or("?", String::as_str);
                let winding = if fact.mirrored { "cw" } else { "ccw" };
                let _ = write!(
                    out,
                    ",\"draw\":{},\"winding\":\"{winding}\",\"submitted\":{}",
                    quote(label),
                    fact.range.len()
                );
                if let Some(v) = variant {
                    let _ = write!(out, ",\"variant\":{v}");
                }
                match kept(facts, read, group, view as usize) {
                    Some(n) if !fact.keep_all => {
                        let _ = write!(out, ",\"culled\":{n}");
                    }
                    _ => out.push_str(",\"culled\":null"),
                }
            }
            Drew::Quads { kind, count } => {
                let _ = write!(out, ",\"draw\":\"{kind}\",\"submitted\":{count}");
            }
            Drew::Blended { batch } => {
                let label = labels.batches.get(batch).map_or("?", String::as_str);
                let _ = write!(
                    out,
                    ",\"draw\":{},\"submitted\":1",
                    quote(&format!("{label} (blended)"))
                );
            }
            Drew::Fixed(name) => {
                let _ = write!(out, ",\"draw\":\"{name}\"");
            }
        }
        if let Some(q) = r.query {
            let _ = write!(
                out,
                ",\"covered\":{}",
                read.covered.get(q as usize) == Some(&true)
            );
        }
        let _ = write!(out, ",\"writes\":\"{}\"}}", writes(r.stage));
        let _ = r;
    }
    out.push(']');
    out
}

fn writes(stage: Stage) -> &'static str {
    match stage {
        Stage::Occluders => "occlusion depth",
        Stage::SunShadow(_) => "sun shadow map",
        Stage::Prepass => "depth",
        Stage::Forward | Stage::Translucent => "colour, depth",
    }
}

/// `state world:<entity> visible`: one word, and the reason.
#[allow(clippy::too_many_arguments)]
pub(crate) fn visible(
    w: &World,
    entity: Entity,
    pending: impl Fn(&str) -> bool,
    camera: Vec3,
    view_proj: glam::Mat4,
    facts: &Facts,
    read: &Reading,
) -> (String, String) {
    let name = |e: Entity| {
        w.name(e)
            .map_or_else(|| format!("#{}", e.index()), str::to_owned)
    };
    let mesh = w.get::<Mesh>(entity).map(|m| (*m).clone());
    if mesh.is_none()
        && !w.has::<exact_game::Sprite>(entity)
        && !w.has::<exact_game::Emitter>(entity)
    {
        return ("not-drawable".into(), "no Mesh, Sprite or Emitter".into());
    }
    if w.get::<exact_game::Visible>(entity).is_some_and(|v| !v.0) {
        return ("hidden".into(), "Visible(false)".into());
    }
    let mut at = entity;
    while let Some(parent) = w.get::<exact_game::Parent>(at).map(|p| p.0) {
        if !w.contains(parent) {
            break;
        }
        if w.get::<exact_game::Visible>(parent).is_some_and(|v| !v.0) {
            return (
                format!("hidden-ancestor {}", name(parent)),
                "an ancestor is Visible(false)".into(),
            );
        }
        at = parent;
    }
    let Some(drawn) = w.drawn(entity) else {
        return ("hidden".into(), "no drawn pose".into());
    };
    if drawn.opacity <= 0. {
        return (
            "opacity 0".into(),
            "Opacity down the Parent chain is zero".into(),
        );
    }
    if let Some(Mesh::Asset(model)) = &mesh {
        if pending(model) {
            return (
                format!("asset-pending {model}"),
                "the model or a texture it uses has not arrived".into(),
            );
        }
    }
    let position = Vec3::from(drawn.pose.translation);
    let distance = position.distance(camera);
    let lod = matches!(mesh, Some(Mesh::Asset(_)))
        .then(|| w.get::<exact_game::ModelLod>(entity))
        .flatten();
    if let Some(lod) = lod {
        if lod.hide.is_some_and(|hide| distance > hide) {
            return (
                format!("lod-hidden {distance:.0} m"),
                format!(
                    "ModelLod hides it beyond {} m",
                    lod.hide.unwrap_or_default()
                ),
            );
        }
    }
    let in_view = in_frustum(view_proj, position, reach(w, entity, &drawn));
    // An entity's slots: its own (primitives) and its model records'.
    let index = entity.index();
    let slots: Vec<u32> = std::iter::once(index)
        .chain(
            (facts.record_slots.iter().enumerate())
                .filter(|(_, &t)| t == index)
                .map(|(r, _)| crate::RENDER_SLOT_BASE + r as u32),
        )
        .collect();
    let groups: Vec<usize> = (facts.groups.iter().enumerate())
        .filter(|(_, g)| {
            facts.slot_list[g.range.start as usize..g.range.end as usize]
                .iter()
                .any(|s| slots.contains(s))
        })
        .map(|(i, _)| i)
        .collect();
    if groups.iter().any(|&g| facts.groups[g].keep_all) {
        return (
            "custom-material".into(),
            "never culled: a game's vertex shader may move it".into(),
        );
    }
    if mesh.is_none() || facts.direct || groups.is_empty() {
        // Quads are culled on the CPU, and a direct frame is not culled at all.
        return if in_view {
            ("drawn".into(), "inside the camera's view".into())
        } else {
            ("frustum".into(), "outside the camera's view".into())
        };
    }
    let kept_in = |v: usize| {
        groups.iter().any(|&g| {
            facts.groups[g].views & (1 << v) != 0
                && list(facts, read, g, v).is_some_and(|l| l.iter().any(|s| slots.contains(s)))
        })
    };
    if !kept_in(0) {
        return if in_view && facts.occluding {
            (
                "occluded".into(),
                "behind last frame's occluders in the depth pyramid".into(),
            )
        } else {
            ("frustum".into(), "outside the camera's view".into())
        };
    }
    let casts = groups.iter().any(|&g| facts.groups[g].views & 0b1110 != 0);
    if facts.cascades > 0 && casts && !(1..=facts.cascades as usize).any(kept_in) {
        return (
            format!("outside-cascade {}", facts.cascades - 1),
            format!(
                "drawn, beyond every sun shadow cascade (0–{})",
                facts.cascades - 1
            ),
        );
    }
    ("drawn".into(), "kept by the camera's cull".into())
}

/// A sphere's radius around the drawn origin that holds the entity's mesh.
fn reach(w: &World, e: Entity, drawn: &exact_game::Drawn) -> f32 {
    let scale = Vec3::from(drawn.pose.matrix3.col(0))
        .length()
        .max(Vec3::from(drawn.pose.matrix3.col(1)).length())
        .max(Vec3::from(drawn.pose.matrix3.col(2)).length());
    let size = match w.get::<Mesh>(e).as_deref() {
        Some(Mesh::Plane { width, depth }) => width.max(*depth),
        Some(Mesh::Box { size }) => size.max_element(),
        Some(Mesh::Sphere { radius }) => radius * 2.,
        Some(Mesh::Capsule { radius, height }) | Some(Mesh::Cylinder { radius, height }) => {
            (radius * 2.).max(*height)
        }
        // Models and quads: a generous metre, as the CPU cannot see their bounds here.
        _ => 2.,
    };
    scale * size * 0.9
}

fn in_frustum(view_proj: glam::Mat4, centre: Vec3, radius: f32) -> bool {
    crate::cull::planes(view_proj).iter().all(|p| {
        p[0] * centre.x + p[1] * centre.y + p[2] * centre.z + p[3]
            >= -radius * Vec3::new(p[0], p[1], p[2]).length()
    })
}

/// Base64 (RFC 4648 §4, padded), for pixels in a JSON reply.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = u32::from(c[0]) << 16
            | u32::from(*c.get(1).unwrap_or(&0)) << 8
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i <= c.len() {
                A[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_name_their_view_and_refuse_bad_numbers() {
        let a = |s: &str| ask(s).unwrap();
        assert_eq!(
            a(r#"{"op":"screenshot","form":"look"}"#),
            Some(Ask::Picture(View::Camera))
        );
        assert_eq!(
            a(r#"{"op":"screenshot","form":"look","yaw":35,"pitch":-20,"distance":40,"now":5}"#),
            Some(Ask::Picture(View::Orbit {
                yaw: 35.,
                pitch: -20.,
                distance: 40.
            }))
        );
        assert_eq!(
            a(r#"{"op":"screenshot","form":"look","top":60}"#),
            Some(Ask::Picture(View::Top { height: 60. }))
        );
        assert!(ask(r#"{"op":"screenshot","form":"look","top":-1}"#).is_err());
        assert_eq!(a(r#"{"op":"state","draws":true}"#), Some(Ask::Draws));
        assert_eq!(
            a(r#"{"op":"state","entity":"tree-7","visible":true}"#),
            Some(Ask::Visible("tree-7".into()))
        );
        assert_eq!(a(r#"{"op":"state","look":true}"#), Some(Ask::Tells));
        assert_eq!(a(r#"{"op":"state","entity":"tree-7"}"#), None);
        assert_eq!(a(r#"{"op":"screenshot","form":"save"}"#), None);
    }

    #[test]
    fn base64_matches_rfc_4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "look_tests.rs"]
mod surface_tests;
