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
    // The shadow tell reads what the shadow passes wrote into the maps.
    let sun_texels = if facts.cascades == 0 {
        Measure::Absent
    } else if read.shadow_texels.len() == facts.cascades as usize {
        Measure::Taken(read.shadow_texels.iter().map(|&n| u64::from(n)).sum())
    } else {
        Measure::Unmeasured("the shadow maps' texels were not read back".into())
    };
    // The caster instances the shadow passes submitted, where the cull counted
    // them (not in a direct frame, which draws casters unculled).
    let sun_casters = (facts.cascades > 0 && !facts.direct)
        .then(|| {
            (records.iter())
                .filter_map(|r| match (r.stage, r.drew) {
                    (Stage::SunShadow(_), Drew::Group { group, view, .. }) => Some((group, view)),
                    _ => None,
                })
                .map(|(group, view)| kept(facts, read, group, view as usize))
                .sum::<Option<u64>>()
        })
        .flatten();
    Drawn {
        particles,
        sun_texels,
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
    /// It is a level of detail the frame does not draw.
    pub level: bool,
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
        hidden: w[5] & 1 != 0,
        level: w[5] & 2 != 0,
    })
}
/// Each model record's box in its entity's space: a rigid one's mesh, and a
/// skinned one deformed by the frame's palette, as the cull bounds it
/// (cull.wgsl `local_sphere`: the union of every joint's transformed mesh
/// sphere, under the node's local matrix).
pub(crate) fn model_boxes(facts: &Facts, read: &Reading) -> Vec<Option<Aabb>> {
    // The bound on a linear map's stretch the cull uses: its 3×3 Frobenius norm.
    let norm = |m: &glam::Mat4| {
        (m.x_axis.truncate().length_squared()
            + m.y_axis.truncate().length_squared()
            + m.z_axis.truncate().length_squared())
        .sqrt()
    };
    let mut boxes = facts.model_boxes.clone();
    for s in &facts.skins {
        let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for j in s.palette..s.palette + s.joints {
            let at = j as usize * 16;
            let Some(words) = read.palette.get(at..at + 16) else {
                continue;
            };
            let joint = glam::Mat4::from_cols_slice(words);
            let (c, r) = (joint.transform_point3(s.centre), s.radius * norm(&joint));
            lo = lo.min(c - Vec3::splat(r));
            hi = hi.max(c + Vec3::splat(r));
        }
        let centre = s.local.transform_point3((lo + hi) * 0.5);
        let r = (hi - lo).length() * 0.5 * norm(&s.local);
        if let Some(b) = boxes.get_mut(s.record as usize) {
            *b = (r.is_finite() && s.joints > 0)
                .then(|| (centre - Vec3::splat(r), centre + Vec3::splat(r)));
        }
    }
    boxes
}

/// The box around every item's sphere in the cull's account (kept-whole
/// groups' included), or `None` without one (a direct frame, no group).
pub(crate) fn culled_bounds(facts: &Facts, read: &Reading) -> Option<Aabb> {
    let mut out: Option<Aabb> = None;
    for (g, fact) in facts.groups.iter().enumerate() {
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
    out.push_str(",\"sunTexels\":");
    match &drawn.sun_texels {
        Measure::Absent => out.push_str("null"),
        Measure::Unmeasured(why) => {
            let _ = write!(out, "{{\"unmeasured\":{}}}", quote(why));
        }
        Measure::Taken(n) => {
            let _ = write!(out, "{n}");
        }
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
            Drew::Quads {
                draw,
                count,
                variant,
                triangles,
                ..
            } => {
                let label = facts.quad_labels.get(draw).map_or("quads", String::as_str);
                let winding = if triangles { "ccw" } else { "camera-facing" };
                let _ = write!(
                    out,
                    ",\"draw\":{},\"winding\":\"{winding}\",\"submitted\":{count},\"variant\":{variant}",
                    quote(label)
                );
                if let Some(k) = facts.counted.iter().position(|&c| c == draw) {
                    match read.counts.get(k) {
                        Some(n) => {
                            let _ = write!(out, ",\"fragments\":{n}");
                        }
                        None => out.push_str(",\"fragments\":null"),
                    }
                }
            }
            Drew::Blended {
                batch: b,
                variant,
                mirrored,
            } => {
                let winding = if mirrored { "cw" } else { "ccw" };
                let _ = write!(
                    out,
                    ",\"draw\":{},\"winding\":\"{winding}\",\"submitted\":1",
                    quote(&format!("{} (blended)", batch(b)))
                );
                if let Some(v) = variant {
                    let _ = write!(out, ",\"variant\":{v}");
                }
            }
            Drew::Hulls { draws } => {
                let _ = write!(
                    out,
                    ",\"draw\":\"outline hulls\",\"draws\":{draws},\"summary\":\"one entry for all its draws\""
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
            None if r.stage.queried()
                && !matches!(
                    r.drew,
                    Drew::Fixed(_) | Drew::Summary(_) | Drew::Hulls { .. }
                ) =>
            {
                out.push_str(",\"covered\":null")
            }
            None => {}
        }
        let _ = write!(out, ",\"writes\":\"{}\"}}", writes(r));
    }
    out.push(']');
    out
}

/// The attachments a draw writes: blended draws and the split translucent
/// pass, whose depth is read-only, write colour alone.
fn writes(r: &Record) -> &'static str {
    match (r.stage, r.drew) {
        (Stage::Occluders, _) => "occlusion depth",
        (Stage::SunShadow(_), _) => "sun shadow map",
        (Stage::LocalShadow(_), _) => "local shadow map",
        (Stage::Prepass, _) => "depth",
        (Stage::Post, _) | (Stage::Translucent, Drew::Quads { .. } | Drew::Blended { .. }) => {
            "colour"
        }
        (_, Drew::Quads { depth: false, .. } | Drew::Blended { .. } | Drew::Fixed("sky")) => {
            "colour"
        }
        _ => "colour, depth",
    }
}

/// `state world:<entity> visible`: one word, and the reason, from what the
/// frame's renderer did with each of the entity's drawables: the quads'
/// decision for a sprite or an emitter, the GPU cull's lists and the CPU's
/// blended cull for a mesh's parts.
pub(crate) fn visible(
    w: &World,
    entity: Entity,
    pending: impl Fn(&str) -> bool,
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
    // Each drawable's own decision: the mesh's (its opaque groups and blended
    // records), the sprite's and the emitter's quads.
    let mut parts: Vec<(&str, (String, String))> = Vec::new();
    if let Some(mesh) = &mesh {
        parts.push(("mesh", mesh_word(entity, mesh, pending, facts, read)));
    }
    for (kind, sprite, has) in [
        ("sprite", true, w.has::<exact_game::Sprite>(entity)),
        ("emitter", false, w.has::<exact_game::Emitter>(entity)),
    ] {
        if has {
            let fact = (facts.quads.iter())
                .find(|(e, _, _, s)| *e == entity && *s == sprite)
                .map(|(_, f, ..)| f);
            parts.push((kind, quad_word(fact)));
        }
    }
    combine(parts)
}

/// Whether a word says the drawable drew.
fn drew(word: &str) -> bool {
    matches!(word, "drawn" | "custom-material") || word.starts_with("outside-cascade")
}

/// An entity's word from its drawables' (`visible`): a mesh that drew keeps its
/// own word; else `drawn` if a sprite or an emitter drew, naming it; else the
/// first one's, naming the others'.
fn combine(parts: Vec<(&str, (String, String))>) -> (String, String) {
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap().1;
    }
    if let Some((_, word)) = parts.iter().find(|(k, w)| *k == "mesh" && drew(&w.0)) {
        return word.clone();
    }
    if let Some((kind, (_, reason))) = parts.iter().find(|(_, w)| drew(&w.0)) {
        return ("drawn".into(), format!("its {kind}: {reason}"));
    }
    let (kind, (word, reason)) = &parts[0];
    let others: Vec<String> = (parts[1..].iter())
        .map(|(k, (w, _))| format!("its {k}: {w}"))
        .collect();
    (
        word.clone(),
        format!("its {kind}: {reason}; {}", others.join("; ")),
    )
}

/// A mesh's word: its model's residency, its level of detail, and the draw
/// groups' and blended records' decisions for its parts.
fn mesh_word(
    entity: Entity,
    mesh: &Mesh,
    pending: impl Fn(&str) -> bool,
    facts: &Facts,
    read: &Reading,
) -> (String, String) {
    if let Mesh::Asset(model) = mesh {
        if pending(model) {
            return (
                format!("asset-pending {model}"),
                "the model or a texture it uses has not arrived".into(),
            );
        }
    }
    // The frame's level selection, from where it drew the entity; it hides
    // records on the paths that read it (below).
    let index = entity.index();
    let lod_hidden = (facts.lods.iter())
        .find(|l| l.slot == index && l.hidden)
        .map(|lod| {
            (
                format!("lod-hidden {:.0} m", lod.distance),
                format!("ModelLod hides it beyond {} m", lod.hide),
            )
        });
    // An entity's slots: its own (primitives) and its model records'.
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
    // Each blended part's decision: the CPU orders and culls them one by one
    // (skipping a hidden level), apart from the draw groups.
    let blended: Vec<bool> = (facts.blended.iter())
        .filter(|(slot, _)| slots.contains(slot))
        .map(|&(_, kept)| kept)
        .collect();
    if groups.is_empty() {
        if let Some(word) = lod_hidden {
            return word;
        }
        return match (blended.is_empty(), blended.contains(&true)) {
            (true, _) => ("not-drawn".into(), "no draw holds it this frame".into()),
            (false, true) => ("drawn".into(), "a blended model the CPU cull kept".into()),
            (false, false) => (
                "frustum".into(),
                "a blended model the CPU cull dropped".into(),
            ),
        };
    }
    // Its opaque parts' word, then every part's: drawn if any part drew.
    let opaque = group_word(facts, read, &groups, &slots, lod_hidden);
    if drew(&opaque.0) || blended.is_empty() {
        return opaque;
    }
    if blended.contains(&true) {
        let reason = format!(
            "its blended part the CPU cull kept; its opaque parts {}: {}",
            opaque.0, opaque.1
        );
        return ("drawn".into(), reason);
    }
    let dropped = blended.len();
    (
        opaque.0,
        format!(
            "{}; its {dropped} blended part{} the CPU cull dropped too",
            opaque.1,
            if dropped == 1 { "" } else { "s" }
        ),
    )
}

/// The word for an entity's parts in the draw groups (`slots` in `groups`):
/// the GPU cull's lists and account, or a direct frame's whole groups.
fn group_word(
    facts: &Facts,
    read: &Reading,
    groups: &[usize],
    slots: &[u32],
    lod_hidden: Option<(String, String)>,
) -> (String, String) {
    let custom = (
        "custom-material".into(),
        "never culled: a game's vertex shader may move it".into(),
    );
    let keep_all = groups.iter().any(|&g| facts.groups[g].keep_all);
    if facts.direct {
        // A direct frame draws each group whole, its finest level's included,
        // and its cull hides no level (`Cull::hide_records`).
        return if keep_all {
            custom
        } else {
            (
                "drawn".into(),
                "submitted unculled: a direct frame tests nothing against the view, \
                 its level of detail included"
                    .into(),
            )
        };
    }
    // The GPU cull drops a hidden level's records.
    if let Some(word) = lod_hidden {
        return word;
    }
    if keep_all {
        return custom;
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
        } else if accounts.iter().any(|a| a.level) {
            (
                "not-drawn".into(),
                "a level of detail the frame does not draw".into(),
            )
        } else {
            (
                "not-drawn".into(),
                "in view and unoccluded by the cull's account, yet not in the camera's list".into(),
            )
        };
    }
    let casts = groups.iter().any(|&g| facts.groups[g].views & 0b1110 != 0);
    if facts.cascades > 0 && casts && !(1..=facts.cascades as usize).any(kept_in) {
        return (
            format!("outside-cascade {}", facts.cascades - 1),
            format!(
                "drawn, in no sun shadow cascade's list (0–{}): past them, or casting where the camera reads none",
                facts.cascades - 1
            ),
        );
    }
    ("drawn".into(), "kept by the camera's cull".into())
}

/// A sprite's or an emitter's word: what the quads did with it this frame.
fn quad_word(fact: Option<&QuadFact>) -> (String, String) {
    match fact {
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
        // The listing's instances: what the shadow passes' records submitted.
        let drew = |records: &[Record]| drawn(records, &facts, &read).sun_casters;
        assert_eq!(drew(&[camera, cascade]), Some(3));
        assert_eq!(drew(&[camera]), Some(0));
        let direct = Facts {
            direct: true,
            ..facts.clone()
        };
        assert_eq!(drawn(&[cascade], &direct, &read).sun_casters, None);
        assert_eq!(
            drawn(&[cascade], &facts, &Reading::default()).sun_casters,
            None
        );
        // The tell reads the maps: a pass that drew (a record, culling intact)
        // but wrote no texel drew no caster.
        let good = crate::tells::FrameStats::of(&[128; 16]);
        let texels = |n: Option<u32>| Reading {
            shadow_texels: n.into_iter().collect(),
            ..read.clone()
        };
        let judged =
            crate::tells::judge(&good, &drawn(&[camera, cascade], &facts, &texels(Some(0))));
        assert_eq!(judged.tells[0].name, "no-shadow-casters");
        let lit = drawn(&[camera, cascade], &facts, &texels(Some(4096)));
        assert_eq!(lit.sun_texels, Measure::Taken(4096));
        assert!(crate::tells::judge(&good, &lit).tells.is_empty());
        let unread = drawn(&[camera, cascade], &facts, &texels(None)).sun_texels;
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
                ..Default::default()
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
    fn the_cull_account_names_occlusion_and_an_undrawn_level_apart() {
        let (facts, _) = culled(1);
        let item = |why: u32| Reading {
            cull: [1f32, 2., 3., 0.5]
                .map(f32::to_bits)
                .into_iter()
                .chain([1, why])
                .collect(),
            ..Reading::default()
        };
        let a = account(&facts, &item(2), 0, 0).unwrap();
        assert!((a.level, a.hidden, a.seen) == (true, false, 1));
        assert_eq!(a.sphere, glam::Vec4::new(1., 2., 3., 0.5));
        let b = account(&facts, &item(1), 0, 0).unwrap();
        assert!(b.hidden && !b.level);
        assert!(
            account(&facts, &item(0), 0, 1).is_none(),
            "past the account's end"
        );
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

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "look_frame_tests.rs"]
mod frame_tests;
