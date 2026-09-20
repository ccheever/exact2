use crate::values::quote;
use crate::{
    json, spatial, Args, Clock, Data, DataError, Entity, Game, Mesh, Parent, Reader, Sim, Vec2,
    Vec3, Visible, World,
};
use std::collections::BTreeMap;

#[derive(Default)]
struct Request {
    op: String,
    entity: Option<String>,
    under: Option<String>,
    to: Option<String>,
    toward: Option<Vec3>,
    from: Option<Vec3>,
    summary: bool,
    pose: bool,
    busy: bool,
    settle: bool,
    now: Option<f64>,
    clock_state: bool,
    ticks: Option<u32>,
    width: Option<f32>,
    height: Option<f32>,
    x: Option<f32>,
    y: Option<f32>,
    since: u64,
    owner: Option<String>,
    reload: bool,
    release_input: bool,
    capture: Option<String>,
    build: String,
    data: String,
    through: Option<u32>,
    limits: crate::CaptureLimits,
    external: bool,
}
impl Request {
    fn parse(text: &str) -> Result<Self, DataError> {
        let mut r = json::Decoder::new(text);
        let mut q = Self::default();
        r.begin_struct()?;
        while let Some(f) = r.field()? {
            match f.as_str() {
                "op" => q.op.read(&mut r)?,
                "clockState" => q.clock_state.read(&mut r)?,
                "owner" => q.owner = Some(r.string()?),
                "reload" => q.reload.read(&mut r)?,
                "releaseInput" => q.release_input.read(&mut r)?,
                "capture" => q.capture = Some(r.string()?),
                "build" => q.build.read(&mut r)?,
                "data" => q.data.read(&mut r)?,
                "through" => {
                    let mut n = 0;
                    n.read(&mut r)?;
                    q.through = Some(n);
                }
                "limits" => q.limits.read(&mut r)?,
                "external" => q.external.read(&mut r)?,
                "entity" => q.entity = Some(r.string()?),
                "under" => q.under = Some(r.string()?),
                "to" => q.to = Some(r.string()?),
                "toward" | "from" => {
                    let mut point = Vec3::ZERO;
                    point.read(&mut r)?;
                    if !point.is_finite() {
                        return Err(DataError::new("route: finite endpoints required"));
                    }
                    if f == "toward" {
                        q.toward = Some(point);
                    } else {
                        q.from = Some(point);
                    }
                }
                "settle" => q.settle.read(&mut r)?,
                "summary" => q.summary.read(&mut r)?,
                "pose" => q.pose.read(&mut r)?,
                "busy" => q.busy.read(&mut r)?,
                "now" => {
                    let mut n = 0.0f64;
                    n.read(&mut r)?;
                    q.now = Some(n);
                }
                "ticks" => {
                    let mut n = 0u32;
                    n.read(&mut r)?;
                    q.ticks = Some(n);
                }
                "width" | "height" | "x" | "y" | "scale" => {
                    let mut n = 0.0f32;
                    n.read(&mut r)?;
                    if !n.is_finite()
                        || (matches!(f.as_str(), "width" | "height" | "scale") && n <= 0.0)
                    {
                        return Err(DataError::new(
                            "expected a positive finite viewport dimension or finite coordinate",
                        )
                        .at(f));
                    }
                    match f.as_str() {
                        "width" => q.width = Some(n),
                        "height" => q.height = Some(n),
                        "x" => q.x = Some(n),
                        "y" => q.y = Some(n),
                        _ => {}
                    }
                }
                "since" => q.since.read(&mut r)?,
                _ => r.skip()?,
            }
        }
        r.finish()?;
        Ok(q)
    }
}
fn encode<T: Data>(v: &T) -> Result<String, String> {
    let mut w = json::Encoder::rounded();
    v.write(&mut w);
    w.finish().map_err(|e| e.to_string())
}
fn resolve(w: &World, name: &str) -> Result<Entity, String> {
    w.resolve(name)
        .ok_or_else(|| format!("no entity named `{name}`; `tree world` lists names; add `w.spawn_named(\"{name}\", (Transform::default(),));` in setup if intended"))
}
fn identity(w: &World, e: Entity) -> String {
    format!(
        "\"id\":{},\"name\":{}",
        e.index(),
        w.name(e).map_or_else(|| "null".into(), quote)
    )
}
fn hierarchy(w: &World) -> Result<Vec<(Entity, Option<Entity>, u32)>, String> {
    let parents: BTreeMap<_, _> = w
        .query::<&Parent>()
        .iter()
        .filter(|(_, p)| w.contains(p.0))
        .map(|(e, p)| (e, p.0))
        .collect();
    let mut children: BTreeMap<Option<Entity>, Vec<Entity>> = BTreeMap::new();
    for e in w.entities() {
        children
            .entry(parents.get(&e).copied())
            .or_default()
            .push(e);
    }
    let mut stack: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|e| (*e, None, 0))
        .collect();
    let mut out = vec![];
    while let Some((e, p, d)) = stack.pop() {
        out.push((e, p, d));
        if let Some(kids) = children.get(&Some(e)) {
            stack.extend(kids.iter().rev().map(|k| (*k, Some(e), d + 1)));
        }
    }
    if out.len() != w.len() {
        let members = parents
            .iter()
            .filter(|(e, _)| !out.iter().any(|(seen, _, _)| seen == *e))
            .map(|(e, p)| format!("{} -> Parent #{}", identity(w, *e), p.index()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!(
            "transform hierarchy contains a cycle: {members}; remove the cyclic Parent in setup"
        ));
    }
    Ok(out)
}
impl<G: Game> Sim<G> {
    /// Renderer-only visibility: a delivered and prepared model and its entire texture closure.
    /// Readiness stays on Sim; game callbacks receive only World.
    /// ```compile_fail
    /// let world = exact_game::World::new(60, 7);
    /// world.model_prepared("hero.model");
    /// ```
    pub fn model_prepared(&self, name: &str) -> bool {
        let ready = |n: &str| {
            self.world.assets.states.get(n) == Some(&crate::asset::AssetState::Loaded)
                && self.world.assets.prepared.contains(n)
                && !self.world.assets.redelivery.contains(n)
        };
        ready(name)
            && self
                .world
                .assets
                .dependencies
                .get(name)
                .is_some_and(|deps| deps.iter().all(|n| ready(n)))
    }
    /// Current renderer requests, including pending dependencies.
    pub fn presentation_assets(&self) -> impl Iterator<Item = &str> {
        self.world.assets.states.keys().map(String::as_str)
    }
    /// Named content failures; readiness must never hide a failed declaration.
    pub fn asset_failures(&self) -> impl Iterator<Item = &str> {
        self.world
            .assets
            .states
            .values()
            .filter_map(|s| match s {
                crate::asset::AssetState::Failed(reason) => Some(reason.as_str()),
                _ => None,
            })
            .chain(
                self.world
                    .assets
                    .refusal
                    .iter()
                    .map(|(_, reason)| reason.as_str()),
            )
    }

    /// Answer the engine half of an agent request as JSON, always tagged with tick.
    /// Component/resource Data uses [] for None and [value] for Some(value).
    pub fn agent(&mut self, request: &str) -> String {
        self.agent_with(request, |_, _| {})
    }
    /// Answer a request, observing the last ticks of any embedded clock advance.
    pub fn agent_with(&mut self, request: &str, after: impl FnMut(&World, u32)) -> String {
        self.agent_with_inspector(request, after, |_, _, pose| {
            if pose {
                Err("pose inspection requires game.assets: true".into())
            } else {
                Ok(String::new())
            }
        })
    }
    /// Extend entity inspection through the linked presentation executor.
    pub fn agent_with_inspector(
        &mut self,
        request: &str,
        after: impl FnMut(&World, u32),
        inspect: impl Fn(&World, Entity, bool) -> Result<String, String>,
    ) -> String {
        match Request::parse(request)
            .map_err(|e| e.to_string())
            .and_then(|q| self.reply(q, after, inspect))
        {
            Ok(reply) => reply,
            Err(error) => {
                self.world.session_log(format_args!("refusal: {error}"));
                format!(
                    "{{\"tick\":{},\"error\":{}}}",
                    self.world.tick(),
                    quote(&error)
                )
            }
        }
    }
    fn reply(
        &mut self,
        q: Request,
        after: impl FnMut(&World, u32),
        inspect: impl Fn(&World, Entity, bool) -> Result<String, String>,
    ) -> Result<String, String> {
        // Imported replay is an isolated calculation, even on a live carrier that
        // attaches a current host timestamp to every inspection request.
        if q.capture.is_some() {
            if q.op != "state" {
                return Err("capture is a form of state".into());
            }
            if q.external {
                return Err("capture refused: unsupported external dependency; world-only replay cannot reproduce hidden I/O".into());
            }
        }
        if q.capture.as_deref() == Some("replay") {
            let bytes = crate::capture::unhex(&q.data)?;
            let capture = crate::Capture::from_bytes(&bytes).map_err(|e| e.to_string())?;
            let mut replay = self
                .replay_capture_using(&capture, &q.build, q.through.map(|n| n as usize))
                .map_err(|e| e.to_string())?;
            return Ok(format!(
                "{{\"tick\":{},\"isolated\":true,\"scope\":\"world-only\",\"replay\":{}}}",
                replay.world.tick(),
                replay.agent("{\"op\":\"state\"}")
            ));
        }
        if q.width.is_some() != q.height.is_some() {
            return Err("width and height must be supplied together".into());
        }
        if q.op == "layout" {
            if let Some((w, h)) = q.width.zip(q.height) {
                self.inspection_viewport = Some(Vec2::new(w, h));
            }
        }
        let viewport = q.width.zip(q.height).map_or(
            self.inspection_viewport.unwrap_or(self.input.viewport),
            |(w, h)| Vec2::new(w, h),
        );
        if q.op == "clock" {
            if let (Some(w), Some(h)) = (q.width, q.height) {
                self.viewport(w, h);
            }
        }
        if q.now.is_some() && q.ticks.is_some() {
            return Err("clock request cannot contain both now and ticks".into());
        }
        if q.ticks.is_some() && q.op != "clock" {
            return Err("ticks belong to the clock operation".into());
        }
        if (q.owner.is_some() || q.reload) && q.op != "clock" {
            return Err("ownership/rebase belongs to clock".into());
        }
        if let Some(owner) = &q.owner {
            if !matches!(owner.as_str(), "human" | "agent") {
                return Err("owner must be human or agent".into());
            }
            self.handoff(owner == "agent");
        }
        if q.reload || q.owner.is_some() {
            if let Some(now) = q.now {
                self.rebase(now, q.release_input)?;
            }
        } else if let Some(now) = q.now.filter(|_| q.op == "clock") {
            self.advance_with(now, Clock::Seekable, after);
        } else if let Some(ticks) = q.ticks {
            if ticks > 216_000 {
                return Err("clock ticks exceeds 216000-step request budget".into());
            }
            if ticks > 0 && G::paused(&self.args) {
                return Err(
                    "clock ticks refused: world is paused; resume through its live binding".into(),
                );
            }
            if self.last_us.is_none() {
                self.advance(0.0, Clock::Seekable);
            }
            let target = self.world.tick().saturating_add(u64::from(ticks));
            let target_us = (u128::from(target) * 1_000_000).div_ceil(u128::from(G::HZ));
            let target_us =
                i64::try_from(target_us).map_err(|_| "clock tick target is too large")?;
            let host_us = self
                .last_us
                .unwrap_or(0)
                .saturating_add(target_us.saturating_sub(self.world_us).max(0));
            self.advance_with(host_us as f64 / 1000.0, Clock::Seekable, after);
        }
        if let Some(command) = &q.capture {
            if q.op != "state" {
                return Err("capture is a form of state".into());
            }
            if q.external {
                return Err("capture refused: unsupported external dependency; world-only replay cannot reproduce hidden I/O".into());
            }
            match command.as_str() {
                "start" => self
                    .start_capture(&q.build, q.limits)
                    .map_err(|e| e.to_string())?,
                "stop" => {
                    self.stop_capture().map_err(|e| e.to_string())?;
                }
                "read" => {}
                _ => return Err("capture expects start, stop, read or replay".into()),
            }
            let capture = self.capture().ok_or("no capture window; start one first")?;
            return Ok(format!(
                "{{\"tick\":{},\"capture\":{}{} }}",
                self.world.tick(),
                capture.status(),
                if matches!(command.as_str(), "stop" | "read") {
                    format!(
                        ",\"data\":{}",
                        quote(&crate::capture::hex(&capture.to_bytes()))
                    )
                } else {
                    String::new()
                }
            ));
        }
        let w = &self.world;
        let tick = w.tick();
        match q.op.as_str() {
            "tree" if q.summary => Ok(format!("{{\"tick\":{tick},\"world\":{{\"name\":{},\"entities\":{},\"tick\":{tick}}}}}", quote(G::NAME), w.len())),
            "tree" => {
                let all = hierarchy(w)?;
                let subtree = q.under.as_deref().map(|n| resolve(w,n)).transpose()?;
                let start = subtree.and_then(|e| all.iter().position(|(a,_,_)| *a == e)).unwrap_or(0);
                let end = if subtree.is_some() { (start+1..all.len()).find(|&i| all[i].2 <= all[start].2).unwrap_or(all.len()) } else { all.len() };
                let entities = all[start..end].iter().take(512).map(|&(e,p,d)| {
                    let names = w.component_names(e).into_iter().map(quote).collect::<Vec<_>>().join(",");
                    format!("{{{},\"parent\":{},\"depth\":{d},\"components\":[{names}],\"tags\":[]}}", identity(w,e), p.map_or_else(|| "null".into(), |e| e.index().to_string()))
                }).collect::<Vec<_>>().join(",");
                Ok(format!("{{\"tick\":{tick},\"entities\":[{entities}],\"truncated\":{}}}", end-start > 512))
            }
            "state" if q.entity.as_deref() == Some("*") => {
                let all = hierarchy(w)?;
                let subtree = q.under.as_deref().map(|n| resolve(w,n)).transpose()?;
                let start = subtree.and_then(|e| all.iter().position(|(a,_,_)| *a == e)).unwrap_or(0);
                let end = if subtree.is_some() { (start+1..all.len()).find(|&i| all[i].2 <= all[start].2).unwrap_or(all.len()) } else { all.len() };
                let entities = all[start..end].iter().take(512).map(|&(e,_,_)| {
                    Ok(format!("{{{},\"components\":{}}}", identity(w,e), w.components_json(e).map_err(|e|e.to_string())?))
                }).collect::<Result<Vec<String>, String>>()?.join(",");
                Ok(format!("{{\"tick\":{tick},\"game\":{},\"hash\":\"0x{:016x}\",\"entities\":[{entities}],\"truncated\":{}{}}}", quote(G::ID), w.hash(), end-start > 512, if q.busy { format!(",\"busy\":{}", encode(&self.changing(self.quiescent()))?) } else { String::new() }))
            }
            "state" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().unwrap())?;
                if q.pose { return Ok(format!("{{\"tick\":{tick},\"entity\":{{{}}},\"pose\":{}}}",identity(w,e),inspect(w,e,true)?)); }
                Ok(format!("{{\"tick\":{tick},\"entity\":{{{},\"components\":{}{}}}}}", identity(w,e), w.components_json(e).map_err(|e|e.to_string())?, inspect(w,e,false)?))
            }
            "state" => {
                let host_input = self.host_input();
                Ok(format!("{{\"tick\":{tick},\"world\":{{\"name\":{},\"game\":{},\"tick\":{tick},\"hz\":{},\"seed\":{},\"hash\":\"0x{:016x}\",\"entities\":{},\"paused\":{},\"loading\":{},\"assets\":{},\"restarted\":{},\"restored\":{}{},\"args\":{},\"resources\":{},\"reload\":{},\"audio\":{},\"ownership\":{},\"capture\":{}{},\"input\":{{\"actions\":{},\"held\":{},\"forwarded\":{},\"controls\":{},\"forwardedControls\":{},\"controlContacts\":{}}},\"published\":{}}}}}",
                quote(G::NAME), quote(G::ID), w.hz(), w.seed(), w.hash(), w.len(), G::paused(&self.args), encode(&w.assets.states.iter().filter(|(_, s)| **s == crate::asset::AssetState::Pending).map(|(n, _)| n.clone()).collect::<Vec<_>>())?, w.assets.state_json(), self.restarted, self.restored, self.restored_from.as_ref().filter(|_| self.restored).map_or_else(String::new, |a| format!(",\"restoredFrom\":{a}")), self.args_json, w.resources_json().map_err(|e|e.to_string())?, self.reload.json(), crate::audio::state(w), self.ownership_json(), self.capture().map_or("null".into(), |c| c.status()), if q.clock_state { format!(",\"clockState\":{{\"hostMicros\":{},\"worldMicros\":{}}}", self.last_us.map_or("null".into(), |n| n.to_string()), self.world_us) } else { String::new() }, self.input.actions().json(), encode(&self.input.keys)?, encode(&host_input.keys)?, encode(&self.input.held_controls())?, encode(&host_input.held_controls())?, encode(&host_input.control_contacts())?, w.published_json(true)))
            },
            "layout" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().ok_or("layout needs an entity")?)?;
                let target = q.to.as_deref().map(|n| resolve(w, n)).transpose()?;
                let mut sight = spatial::index::Sight::new(w, e, target, None)?;
                let mut result = self.layout_json(e, viewport, target, &mut sight)?;
                let toward = q.toward.or_else(|| target.and_then(|e| sight.pose(e).map(|p| p.translation.into())));
                if let Some(toward) = toward {
                    let from = q.from.or_else(|| sight.pose(e).map(|p| p.translation.into()));
                    let route = match from {
                        Some(from) => self.route_with_sight(&mut sight, from, toward)?,
                        None => r#"{"unavailable":"subject global pose unavailable"}"#.into(),
                    };
                    result.pop();
                    result.push_str(&format!(",\"route\":{route}}}"));
                }
                Ok(result)
            }
            "layout" => {
                spatial::index::check_size(w)?;
                let point = Vec2::new(q.x.ok_or("layout needs x")?,q.y.ok_or("layout needs y")?);
                let view = spatial::view(w,viewport)?.ok_or("layout unavailable: needs an active camera and viewport; add `w.spawn_named(\"camera\", (Transform::at(0., 3., 8.), Camera::default()));` in setup; inspect `state world:*`")?;
                let hit = spatial::pick(w,&view,point).map(|(e,d,p)| Ok::<_,String>(format!("{{{},\"distance\":{},\"point\":{}}}", identity(w,e),encode(&d)?,encode(&p)?))).transpose()?.unwrap_or_else(||"null".into());
                Ok(format!("{{\"tick\":{tick},\"hit\":{hit}}}"))
            }
            "clock" if self.is_loading() => {
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":false,\"changing\":[\"loading\"],\"error\":{},\"assets\":{}}}", w.hash(), quote(&format!("clock refused: declared assets are not ready: {}; inspect untargeted `state`: world[0].loading and world[0].assets", w.assets.state_json())), w.assets.state_json()))
            }
            "clock" => {
                if q.reload {
                    let values = self.args.values().iter().map(|v| crate::values::value_json(v, false)).collect::<Vec<_>>().join(",");
                    let names = G::Args::FIELDS.iter().map(|(name, _)| quote(name)).collect::<Vec<_>>().join(",");
                    let setup = G::Args::FIELDS.iter().enumerate().filter(|(_, (_, kind))| *kind == crate::ArgumentKind::Setup).map(|(i, _)| i.to_string()).collect::<Vec<_>>().join(",");
                    return Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"reload\":{{\"rebased\":true,\"releasedInput\":{},\"values\":[{values}],\"names\":[{names}],\"setupIndices\":[{setup}]}},\"ownership\":{}}}", w.hash(), q.release_input, self.ownership_json()));
                }
                let quiescent = self.quiescent();
                let deadline = if quiescent { String::new() } else { format!(",\"settleAt\":{}", crate::data::text::Float(self.settle_at(q.settle))) };
                let changing = encode(&self.changing(quiescent))?;
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":{quiescent},\"changing\":{changing},\"ownership\":{}{deadline}}}", w.hash(), self.ownership_json()))
            },
            "logs" => {
                let lines = w.journal(); let next = w.journal_next(); let start = lines.first().map_or(next, |e|e.index); let from = q.since.clamp(start,next);
                let lines = lines.iter().filter(|e|e.index >= from).map(|e|quote(&e.line)).collect::<Vec<_>>().join(",");
                Ok(format!("{{\"tick\":{tick},\"next\":{next},\"from\":{from},\"lines\":[{lines}]}}"))
            }
            _ => Err(format!("unknown op `{}`; use tree, screenshot, tap, type, state, layout, logs or clock",q.op)),
        }
    }
    fn ownership_json(&self) -> String {
        format!("{{\"owner\":{},\"clock\":{},\"contamination\":{},\"inputSource\":{},\"handoff\":\"clock owner human/agent\"}}", quote(if self.agent_owned { "agent" } else { "human" }), quote(if self.agent_owned { "controlled" } else { "live" }), self.contamination, quote(if self.source_tagged { "attested" } else { "unavailable" }))
    }
    fn layout_json(
        &self,
        e: Entity,
        viewport: Vec2,
        to: Option<Entity>,
        sight: &mut spatial::index::Sight<'_>,
    ) -> Result<String, String> {
        let w = &self.world;
        spatial::index::check_size(w)?;
        let Some(pose) = sight.pose(e).filter(|p| p.is_finite()) else {
            let screen = spatial::layout(w, viewport, e)
                .screen
                .map(|[x, y, w, h]| {
                    Ok::<_, String>(format!(
                        "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                        encode(&x)?,
                        encode(&y)?,
                        encode(&w)?,
                        encode(&h)?
                    ))
                })
                .transpose()?
                .unwrap_or_else(|| "{\"unavailable\":true}".into());
            return Ok(format!("{{\"tick\":{},\"entity\":{{{},\"world\":null,\"bounds\":null,\"screen\":{screen},\"depth\":null,\"visible\":{{\"unavailable\":true,\"reason\":\"missing global pose\"}},\"facing\":{{\"forward\":null,\"towardCamera\":null,\"bearingTo\":null,\"distanceTo\":null,\"lineOfSight\":null}}}}}}", w.tick(), identity(w, e)));
        };
        let position: Vec3 = pose.translation.into();
        let (scale, rotation) = if pose.matrix3.determinant().abs() < 1e-12 {
            (
                Vec3::new(
                    pose.matrix3.x_axis.length(),
                    pose.matrix3.y_axis.length(),
                    pose.matrix3.z_axis.length(),
                ),
                "null".into(),
            )
        } else {
            let (scale, rotation, _) = pose.to_scale_rotation_translation();
            (scale, encode(&rotation)?)
        };
        let mesh = w.get::<Mesh>(e);
        let unbounded = spatial::unbounded(w, e, mesh.as_deref());
        let (half, center) = spatial::bounds(w, e, mesh.as_deref());
        let corners = spatial::corners(pose, half, center);
        let lo = corners
            .iter()
            .copied()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let hi = corners
            .iter()
            .copied()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let forward = pose.transform_vector3(Vec3::NEG_Z).try_normalize();
        let view = sight.view(viewport);
        let toward = view.as_ref().and_then(|v| {
            forward.map(|f| f.dot((Vec3::from(v.pose.translation) - position).normalize_or_zero()))
        });
        let mut facing = format!(
            "\"forward\":{},\"towardCamera\":{}",
            forward
                .as_ref()
                .map(encode)
                .transpose()?
                .unwrap_or_else(|| "null".into()),
            toward
                .map(|n| encode(&n))
                .transpose()?
                .unwrap_or_else(|| "null".into())
        );
        if let Some(to) = to {
            if let Some(target) = sight.pose(to).filter(|p| p.is_finite()) {
                let delta: Vec3 = Vec3::from(target.translation) - position;
                let bearing = forward.map(|forward| {
                    if (forward.x == 0.0 && forward.z == 0.0) || (delta.x == 0.0 && delta.z == 0.0)
                    {
                        0.0
                    } else {
                        let degrees = crate::math::atan2(
                            forward.z * delta.x - forward.x * delta.z,
                            forward.x * delta.x + forward.z * delta.z,
                        )
                        .to_degrees();
                        let degrees = (degrees * 10000.0).round() / 10000.0;
                        if degrees == -180.0 {
                            180.0
                        } else {
                            degrees
                        }
                    }
                });
                let clear = !sight.segment(position, target.translation.into(), 1, |_, _| true)?;
                facing.push_str(&format!(
                    ",\"bearingTo\":{},\"distanceTo\":{},\"lineOfSight\":{clear}",
                    bearing
                        .as_ref()
                        .map(encode)
                        .transpose()?
                        .unwrap_or_else(|| "null".into()),
                    encode(&delta.length())?
                ));
            } else {
                facing.push_str(",\"bearingTo\":null,\"distanceTo\":null,\"lineOfSight\":null,\"reason\":\"target global pose unavailable\"");
            }
        }
        let (screen, depth, visible) = if let Some(view) = view.filter(|_| !unbounded) {
            let screen = view
                .screen(&corners)
                .map(|[x, y, width, height]| {
                    Ok::<_, String>(format!(
                        "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
                        encode(&x)?,
                        encode(&y)?,
                        encode(&width)?,
                        encode(&height)?
                    ))
                })
                .transpose()?
                .unwrap_or_else(|| "{\"unavailable\":true}".into());
            let (inside, behind, distance, depth) = view.visibility(&corners, position);
            let reason = if behind {
                Some("behind camera")
            } else if !inside {
                Some("outside frustum")
            } else if w.get::<Visible>(e).is_some_and(|v| !v.0) {
                Some("hidden")
            } else {
                None
            };
            let (occluded, occluders) = if reason.is_none() {
                let (fraction, names) =
                    spatial::occlusion(sight, view.pose.translation.into(), &corners)?;
                (Some(fraction), names)
            } else {
                (None, Vec::new())
            };
            let reason = reason.map_or(String::new(), |r| format!(",\"reason\":{}", quote(r)));
            let names: Vec<_> = occluders
                .into_iter()
                .map(|e| {
                    w.name(e)
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("#{}", e.index()))
                })
                .collect();
            (
                screen,
                encode(&depth)?,
                format!(
                    "{{\"inFrustum\":{},\"behindCamera\":{behind},\"distance\":{},\"occluded\":{},\"occluders\":{}{reason}}}",
                    inside && w.get::<Visible>(e).is_none_or(|v| v.0),
                    encode(&distance)?, occluded.as_ref().map(encode).transpose()?.unwrap_or_else(|| "null".into()), encode(&names)?
                ),
            )
        } else {
            (
                "{\"unavailable\":true}".into(),
                "null".into(),
                "{\"unavailable\":true}".into(),
            )
        };
        let bounds = if unbounded {
            "null".into()
        } else {
            format!("{{\"min\":{},\"max\":{}}}", encode(&lo)?, encode(&hi)?)
        };
        Ok(format!("{{\"tick\":{},\"entity\":{{{},\"world\":{{\"position\":{},\"rotation\":{},\"scale\":{}}},\"bounds\":{bounds},\"screen\":{screen},\"depth\":{depth},\"visible\":{visible},\"facing\":{{{facing}}}}}}}", w.tick(),identity(w,e),json::to_string(&position).map_err(|e| e.to_string())?,rotation,encode(&scale)?))
    }
}
