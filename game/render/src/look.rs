//! Look frames' requests and answers (LLP 1127.000 D1, D3, D4): the agent's
//! `screenshot … world from …`, the tells at `clock settle` and `screenshot`,
//! `state world draws` and `state world:<entity> visible`. Each is answered
//! from one look frame, the current tick drawn offscreen (`surface/look.rs`),
//! and only under the agent's clock.
use crate::look_bounds::Aabb;
use crate::quads::QuadFact;
use crate::renderer::probe::{Drew, Facts, Reading, Record, Stage};
use crate::tells::{Drawn, FrameStats, Judgement, Measure, ParticleDraw};
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

impl View {
    /// Point `input` at this view of the scene within `bounds` (its centre and
    /// radius, `look_bounds::scene`); the game's camera leaves it as it is.
    pub(crate) fn apply(self, input: &mut crate::FrameInput<'_>, size: Vec2, bounds: (Vec3, f32)) {
        let (centre, radius) = bounds;
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

/// The tells' inputs that are not pixels: each particle draw's counted
/// fragments, and the casters a shadowed sun's shadow passes drew. A
/// measurement the frame did not take is `Unmeasured`, never zero.
pub(crate) fn drawn(records: &[Record], facts: &Facts, read: &Reading) -> Drawn {
    let particles = if facts.particle_draws.is_empty() {
        Measure::Absent
    } else {
        let counted = (facts.particle_draws.iter())
            .map(|&(draw, live)| {
                let k = facts.counted.iter().position(|&c| c == draw)?;
                Some(ParticleDraw {
                    batch: facts.quad_labels.get(draw).cloned().unwrap_or_default(),
                    live: u64::from(live),
                    covered: u64::from(*read.counts.get(k)?),
                })
            })
            .collect::<Option<Vec<_>>>();
        let why = facts.uncounted.unwrap_or("the counts were not read back");
        counted.map_or_else(|| Measure::Unmeasured(why.into()), Measure::Taken)
    };
    let sun_casters = if facts.cascades == 0 {
        Measure::Absent
    } else {
        let mut casters = Some(0);
        let mut why = "";
        // Only what the shadow passes submitted: the cull's lists for a view
        // count nothing a pass did not draw.
        for r in records {
            let (Stage::SunShadow(_), Drew::Group { group, view, .. }) = (r.stage, r.drew) else {
                continue;
            };
            let n = if facts.direct {
                why = "a direct frame draws casters unculled, uncounted per cascade";
                None
            } else {
                why = "the cull's counts were not read back";
                kept(facts, read, group, view as usize)
            };
            casters = casters.zip(n).map(|(a, b)| a + b);
        }
        casters.map_or_else(|| Measure::Unmeasured(why.into()), Measure::Taken)
    };
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

/// Instances local shadow cull `c`'s group `g` kept in view `v`, in a culled frame.
fn local_kept(facts: &Facts, read: &Reading, c: usize, g: usize, v: usize) -> Option<u64> {
    let fact = facts.local.get(c)?.groups.get(g)?;
    let at = (fact.offsets[v].1 / 4) as usize + 1;
    let words = read.local_indirect.get(c).filter(|_| !facts.direct)?;
    words.get(at).map(|&n| u64::from(n))
}

/// The cull's account of one item (cull.wgsl `probe`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Account {
    /// Its world sphere: centre and radius, swept between ticks, skinned.
    pub sphere: glam::Vec4,
    /// Views whose frustum holds it (bit 0 the camera).
    pub seen: u32,
    /// The camera's occlusion pyramid hides it.
    pub hidden: bool,
}
/// Group `g`'s item `local`, from the cull's account of the frame.
pub(crate) fn account(facts: &Facts, read: &Reading, g: usize, local: u32) -> Option<Account> {
    let words = crate::cull::PROBE_WORDS as usize;
    let at = (facts.groups.get(g)?.chunk as usize * 64 + local as usize) * words;
    let w = read.cull.get(at..at + words)?;
    let f = |i: usize| f32::from_bits(w[i]);
    Some(Account {
        sphere: glam::Vec4::new(f(0), f(1), f(2), f(3)),
        seen: w[4],
        hidden: w[5] != 0,
    })
}
/// The box around every item's sphere in the cull's account, or `None`
/// without one (a direct frame, no culled group).
pub(crate) fn culled_bounds(facts: &Facts, read: &Reading) -> Option<Aabb> {
    let mut out: Option<Aabb> = None;
    for (g, fact) in facts.groups.iter().enumerate().filter(|(_, f)| !f.keep_all) {
        for local in 0..fact.range.len() as u32 {
            let Some(a) = account(facts, read, g, local) else {
                continue;
            };
            let (c, r) = (a.sphere.truncate(), a.sphere.w);
            if c.is_finite() && r.is_finite() && r > 0. {
                let b = (c - Vec3::splat(r), c + Vec3::splat(r));
                out = Some(out.map_or(b, |(lo, hi)| (lo.min(b.0), hi.max(b.1))));
            }
        }
    }
    out
}

/// The slots group `g` kept in view `v`.
fn list<'a>(facts: &Facts, read: &'a Reading, g: usize, v: usize) -> Option<&'a [u32]> {
    let n = kept(facts, read, g, v)? as usize;
    let at = (facts.groups[g].offsets[v].0 / 4) as usize;
    read.lists.get(at..at + n)
}

/// The tells' JSON: statistics, tells, the measurements not taken, what the
/// particle and shadow tells read, and cost.
pub(crate) fn tells_json(
    stats: &FrameStats,
    drawn: &Drawn,
    judged: &Judgement,
    bytes: u64,
) -> String {
    let mut out = format!(
        "{{\"mean\":{},\"clipped\":{},\"histogram\":{:?},\"tells\":[",
        round(stats.mean),
        round(stats.clipped),
        stats.histogram
    );
    for (i, t) in judged.tells.iter().enumerate() {
        let comma = if i == 0 { "" } else { "," };
        let _ = write!(
            out,
            "{comma}{{\"name\":\"{}\",\"line\":{}}}",
            t.name,
            quote(&t.line)
        );
    }
    out.push_str("],\"gaps\":[");
    for (i, gap) in judged.gaps.iter().enumerate() {
        out.push_str(if i == 0 { "" } else { "," });
        out.push_str(&quote(gap));
    }
    out.push(']');
    out.push_str(",\"particles\":");
    match &drawn.particles {
        Measure::Absent => out.push_str("null"),
        Measure::Unmeasured(why) => {
            let _ = write!(out, "{{\"unmeasured\":{}}}", quote(why));
        }
        Measure::Taken(draws) => {
            out.push('[');
            for (i, d) in draws.iter().enumerate() {
                let _ = write!(
                    out,
                    "{}{{\"batch\":{},\"live\":{},\"covered\":{}}}",
                    if i == 0 { "" } else { "," },
                    quote(&d.batch),
                    d.live,
                    d.covered
                );
            }
            out.push(']');
        }
    }
    out.push_str(",\"sunCasters\":");
    match &drawn.sun_casters {
        Measure::Absent => out.push_str("null"),
        Measure::Unmeasured(why) => {
            let _ = write!(out, "{{\"unmeasured\":{}}}", quote(why));
        }
        Measure::Taken(n) => {
            let _ = write!(out, "{n}");
        }
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
/// submitted and kept after culling, whether it covered a sample (`null` where
/// its pass is queried and the draw was not measured), and what it wrote.
pub(crate) fn draws_json(
    records: &[Record],
    facts: &Facts,
    read: &Reading,
    labels: &Labels,
) -> String {
    let mut out = String::from("[");
    let batch = |b: usize| labels.batches.get(b).map_or("?", String::as_str);
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
                let winding = if fact.mirrored { "cw" } else { "ccw" };
                let _ = write!(
                    out,
                    ",\"draw\":{},\"winding\":\"{winding}\",\"submitted\":{}",
                    quote(batch(fact.batch)),
                    fact.range.len()
                );
                if let Some(v) = variant {
                    let _ = write!(out, ",\"variant\":{v}");
                }
                match kept(facts, read, group, view as usize) {
                    Some(n) if !fact.keep_all => {
                        let _ = write!(out, ",\"kept\":{n}");
                    }
                    _ => out.push_str(",\"kept\":null"),
                }
            }
            Drew::Quads { draw, count } => {
                let label = facts.quad_labels.get(draw).map_or("quads", String::as_str);
                let _ = write!(out, ",\"draw\":{},\"submitted\":{count}", quote(label));
                if let Some(k) = facts.counted.iter().position(|&c| c == draw) {
                    match read.counts.get(k) {
                        Some(n) => {
                            let _ = write!(out, ",\"fragments\":{n}");
                        }
                        None => out.push_str(",\"fragments\":null"),
                    }
                }
            }
            Drew::Blended { batch: b } => {
                let _ = write!(
                    out,
                    ",\"draw\":{},\"submitted\":1",
                    quote(&format!("{} (blended)", batch(b)))
                );
            }
            Drew::Local {
                cull,
                group,
                view,
                variant,
            } => {
                let fact = facts.local.get(cull).and_then(|l| l.groups.get(group));
                let (label, submitted) = fact.map_or(("?", 0), |f| (batch(f.batch), f.range.len()));
                let winding = if fact.is_some_and(|f| f.mirrored) {
                    "cw"
                } else {
                    "ccw"
                };
                let _ = write!(
                    out,
                    ",\"draw\":{},\"winding\":\"{winding}\",\"submitted\":{submitted}",
                    quote(label)
                );
                if let Some(v) = variant {
                    let _ = write!(out, ",\"variant\":{v}");
                }
                match local_kept(facts, read, cull, group, view as usize) {
                    Some(n) => {
                        let _ = write!(out, ",\"kept\":{n}");
                    }
                    None => out.push_str(",\"kept\":null"),
                }
            }
            Drew::Summary(name) => {
                let _ = write!(
                    out,
                    ",\"draw\":\"{name}\",\"summary\":\"one entry for all its draws\""
                );
            }
            Drew::Fixed(name) => {
                let _ = write!(out, ",\"draw\":\"{name}\"");
            }
        }
        match r.query {
            Some(q) => match read.covered.get(q as usize) {
                Some(c) => {
                    let _ = write!(out, ",\"covered\":{c}");
                }
                None => out.push_str(",\"covered\":null"),
            },
            None if r.stage.queried() && !matches!(r.drew, Drew::Fixed(_) | Drew::Summary(_)) => {
                out.push_str(",\"covered\":null")
            }
            None => {}
        }
        let _ = write!(out, ",\"writes\":\"{}\"}}", writes(r.stage));
    }
    out.push(']');
    out
}

fn writes(stage: Stage) -> &'static str {
    match stage {
        Stage::Occluders => "occlusion depth",
        Stage::SunShadow(_) => "sun shadow map",
        Stage::LocalShadow(_) => "local shadow map",
        Stage::Prepass => "depth",
        Stage::Forward | Stage::Translucent => "colour, depth",
        Stage::Post => "colour",
    }
}

/// `state world:<entity> visible`: one word, and the reason, from what the
/// frame's renderer did: the quads' decision for a sprite or an emitter, the
/// GPU cull's lists for a mesh.
#[allow(clippy::too_many_arguments)]
pub(crate) fn visible(
    w: &World,
    entity: Entity,
    pending: impl Fn(&str) -> bool,
    camera: Vec3,
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
    if mesh.is_none() {
        return quad_word(facts, entity);
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
    if groups.is_empty() {
        let blended: Vec<bool> = (facts.blended.iter())
            .filter(|(slot, _)| slots.contains(slot))
            .map(|&(_, kept)| kept)
            .collect();
        return match (blended.is_empty(), blended.contains(&true)) {
            (true, _) => ("not-drawn".into(), "no draw holds it this frame".into()),
            (false, true) => ("drawn".into(), "a blended model the CPU cull kept".into()),
            (false, false) => (
                "frustum".into(),
                "a blended model the CPU cull dropped".into(),
            ),
        };
    }
    if groups.iter().any(|&g| facts.groups[g].keep_all) {
        return (
            "custom-material".into(),
            "never culled: a game's vertex shader may move it".into(),
        );
    }
    if facts.direct {
        return (
            "drawn".into(),
            "submitted unculled: a direct frame tests nothing against the view".into(),
        );
    }
    let kept_in = |v: usize| {
        groups.iter().any(|&g| {
            facts.groups[g].views & (1 << v) != 0
                && list(facts, read, g, v).is_some_and(|l| l.iter().any(|s| slots.contains(s)))
        })
    };
    if !kept_in(0) {
        // The cull's own account of the entity's items says which test dropped it.
        let slots = &slots;
        let accounts: Vec<Account> = (groups.iter())
            .flat_map(|&g| {
                let range = facts.groups[g].range.clone();
                (range.clone()).filter_map(move |at| {
                    let local = at - range.start;
                    slots
                        .contains(&facts.slot_list[at as usize])
                        .then(|| account(facts, read, g, local))
                        .flatten()
                })
            })
            .collect();
        return if accounts.is_empty() {
            (
                "not-drawn".into(),
                "dropped by the camera's cull; its account was not read back".into(),
            )
        } else if accounts.iter().any(|a| a.seen & 1 != 0 && a.hidden) {
            (
                "occluded".into(),
                "behind last frame's occluders in the depth pyramid".into(),
            )
        } else if accounts.iter().all(|a| a.seen & 1 == 0) {
            ("frustum".into(), "outside the camera's view".into())
        } else {
            (
                "not-drawn".into(),
                "inside the view and unoccluded, at a level of detail not drawn this frame".into(),
            )
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

/// A sprite's or an emitter's word: what the quads did with it this frame.
fn quad_word(facts: &Facts, entity: Entity) -> (String, String) {
    match facts
        .quads
        .iter()
        .find(|(e, _)| *e == entity)
        .map(|(_, f)| f)
    {
        Some(QuadFact::Pending(texture)) => (
            format!("asset-pending {texture}"),
            "its texture is not resident".into(),
        ),
        Some(QuadFact::Culled) => ("frustum".into(), "outside the camera's view".into()),
        Some(QuadFact::Drawn(0)) => (
            "no-particles".into(),
            "the emitter has no live particle this frame".into(),
        ),
        Some(QuadFact::Drawn(n)) => ("drawn".into(), format!("drew {n} quads")),
        None => (
            "not-drawn".into(),
            "the frame's quads did not reach it (sprites draw only in a model-capable module)"
                .into(),
        ),
    }
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

    /// One culled group in the camera and cascade 0, its cull keeping `kept`
    /// instances in each.
    fn culled(kept: u32) -> (Facts, Reading) {
        let views = crate::cull::VIEWS as usize;
        let facts = Facts {
            groups: vec![crate::renderer::probe::GroupFact {
                batch: 0,
                range: 0..4,
                views: 0b11,
                keep_all: false,
                mirrored: false,
                offsets: std::array::from_fn(|v| (0, v as u64 * 20)),
                chunk: 0,
            }],
            cascades: 1,
            ..Facts::default()
        };
        let indirect = (0..views).flat_map(|_| [36, kept, 0, 0, 0]).collect();
        let read = Reading {
            indirect,
            ..Reading::default()
        };
        (facts, read)
    }
    fn record(stage: Stage, drew: Drew) -> Record {
        Record {
            stage,
            drew,
            query: None,
        }
    }

    #[test]
    fn casters_are_what_the_shadow_pass_drew_never_what_the_cull_kept() {
        let (facts, read) = culled(3);
        let cascade = record(
            Stage::SunShadow(0),
            Drew::Group {
                group: 0,
                view: 1,
                variant: None,
            },
        );
        let camera = record(
            Stage::Forward,
            Drew::Group {
                group: 0,
                view: 0,
                variant: None,
            },
        );
        let drew = |records: &[Record]| drawn(records, &facts, &read).sun_casters;
        assert_eq!(drew(&[camera, cascade]), Measure::Taken(3));
        // Shadow drawing suppressed, culling intact: the cull kept three
        // casters for the cascade, and the pass drew none.
        assert_eq!(drew(&[camera]), Measure::Taken(0));
        let judged = crate::tells::judge(
            &crate::tells::FrameStats::of(&[128; 16]),
            &drawn(&[camera], &facts, &read),
        );
        assert_eq!(judged.tells[0].name, "no-shadow-casters");
        // A direct frame draws its casters unculled: unknown, not invented.
        let direct = Facts {
            direct: true,
            ..facts.clone()
        };
        let unknown = drawn(&[cascade], &direct, &read).sun_casters;
        assert!(matches!(unknown, Measure::Unmeasured(_)), "{unknown:?}");
        // And a cull whose counts were not read back is unknown too.
        let unread = drawn(&[cascade], &facts, &Reading::default()).sun_casters;
        assert!(matches!(unread, Measure::Unmeasured(_)), "{unread:?}");
    }

    #[test]
    fn particle_draws_are_judged_one_by_one_and_uncounted_draws_are_gaps() {
        let facts = Facts {
            quad_labels: vec![
                "particle dots · additive".into(),
                "sprite a.tex".into(),
                "particles smoke.tex · alpha".into(),
            ],
            particle_draws: vec![(0, 40), (2, 12)],
            counted: vec![0, 2],
            ..Facts::default()
        };
        let read = Reading {
            counts: vec![900, 0],
            ..Reading::default()
        };
        let Measure::Taken(draws) = drawn(&[], &facts, &read).particles else {
            panic!("counted draws are taken");
        };
        assert_eq!((draws[0].live, draws[0].covered), (40, 900));
        assert_eq!(draws[1].batch, "particles smoke.tex · alpha");
        // One covered draw does not hide the one that covered nothing.
        let judged = crate::tells::judge(
            &crate::tells::FrameStats::of(&[128; 16]),
            &Drawn {
                particles: Measure::Taken(draws),
                sun_casters: Measure::Absent,
            },
        );
        assert_eq!(judged.tells.len(), 1);
        assert!(judged.tells[0].line.contains("smoke.tex"), "{judged:?}");
        // Draws the counting pass did not count are not measured.
        let uncounted = drawn(&[], &facts, &Reading::default()).particles;
        assert!(matches!(uncounted, Measure::Unmeasured(_)), "{uncounted:?}");
        let none = Facts::default();
        assert_eq!(drawn(&[], &none, &read).particles, Measure::Absent);
    }

    #[test]
    fn a_draw_the_queries_did_not_reach_lists_as_unmeasured() {
        let (facts, read) = culled(2);
        let forward = |query| Record {
            stage: Stage::Forward,
            drew: Drew::Group {
                group: 0,
                view: 0,
                variant: None,
            },
            query,
        };
        let read = Reading {
            covered: vec![true],
            ..read
        };
        let labels = Labels {
            batches: vec!["cube".into()],
        };
        let json = draws_json(&[forward(Some(0)), forward(None)], &facts, &read, &labels);
        let listed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(listed[0]["covered"], true, "{json}");
        assert!(listed[1]["covered"].is_null() && json.matches("\"covered\"").count() == 2);
        assert_eq!(listed[1]["kept"], 2, "{json}");
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
