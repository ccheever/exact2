use crate::values::quote;
use crate::{
    json, spatial, Args, Clock, Data, DataError, Entity, Game, Parent, Reader, Sim, Vec2, Vec3,
    Visible, World,
};
use std::collections::BTreeMap;

#[derive(Default)]
struct Request {
    op: String,
    entity: Option<String>,
    under: Option<String>,
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
                self.world.log(format_args!("refusal: {error}"));
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
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"entities\":[{entities}],\"truncated\":{}{}}}", w.hash(), end-start > 512, if q.busy { format!(",\"busy\":{}", encode(&self.changing(self.quiescent()))?) } else { String::new() }))
            }
            "state" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().unwrap())?;
                if q.pose { return Ok(format!("{{\"tick\":{tick},\"entity\":{{{}}},\"pose\":{}}}",identity(w,e),inspect(w,e,true)?)); }
                Ok(format!("{{\"tick\":{tick},\"entity\":{{{},\"components\":{}{}}}}}", identity(w,e), w.components_json(e).map_err(|e|e.to_string())?, inspect(w,e,false)?))
            }
            "state" => {
                let host_input = self.host_input();
                Ok(format!("{{\"tick\":{tick},\"world\":{{\"name\":{},\"tick\":{tick},\"hz\":{},\"seed\":{},\"hash\":\"0x{:016x}\",\"entities\":{},\"paused\":{},\"loading\":{},\"assets\":{},\"restarted\":{},\"restored\":{}{},\"args\":{},\"resources\":{},\"audio\":{},\"ownership\":{},\"capture\":{}{},\"input\":{{\"actions\":{},\"held\":{},\"forwarded\":{},\"controls\":{},\"forwardedControls\":{},\"controlContacts\":{}}},\"published\":{}}}}}",
                quote(G::NAME), w.hz(), w.seed(), w.hash(), w.len(), G::paused(&self.args), encode(&w.assets.states.iter().filter(|(_, s)| **s == crate::asset::AssetState::Pending).map(|(n, _)| n.clone()).collect::<Vec<_>>())?, w.assets.state_json(), self.restarted, self.restored, self.restored_from.as_ref().filter(|_| self.restored).map_or_else(String::new, |a| format!(",\"restoredFrom\":{a}")), self.args_json, w.resources_json().map_err(|e|e.to_string())?, crate::audio::state(w), self.ownership_json(), self.capture().map_or("null".into(), |c| c.status()), if q.clock_state { format!(",\"clockState\":{{\"hostMicros\":{},\"worldMicros\":{}}}", self.last_us.map_or("null".into(), |n| n.to_string()), self.world_us) } else { String::new() }, self.input.actions().json(), encode(&self.input.keys)?, encode(&host_input.keys)?, encode(&self.input.held_controls())?, encode(&host_input.held_controls())?, encode(&host_input.control_contacts())?, w.published_json(true)))
            },
            "layout" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().ok_or("layout needs an entity")?)?;
                self.layout_json(e, viewport)
            }
            "layout" => {
                let point = Vec2::new(q.x.ok_or("layout needs x")?,q.y.ok_or("layout needs y")?);
                let view = spatial::View::new(w,viewport).ok_or("layout unavailable: needs an active camera and viewport; add `w.spawn_named(\"camera\", (Transform::at(0., 3., 8.), Camera::default()));` in setup; inspect `state world:*`")?;
                let hit = spatial::pick(w,&view,point).map(|(e,d,p)| Ok::<_,String>(format!("{{{},\"distance\":{},\"point\":{}}}", identity(w,e),encode(&d)?,encode(&p)?))).transpose()?.unwrap_or_else(||"null".into());
                Ok(format!("{{\"tick\":{tick},\"hit\":{hit}}}"))
            }
            "clock" if self.is_loading() => {
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":false,\"changing\":[\"loading\"],\"error\":{},\"assets\":{}}}", w.hash(), quote(&format!("clock refused: declared assets are not ready: {}; inspect untargeted `state`: world[0].loading and world[0].assets", w.assets.state_json())), w.assets.state_json()))
            }
            "clock" => {
                if q.reload {
                    let values = self.args.values().iter().map(|v| crate::values::value_json(v, false)).collect::<Vec<_>>().join(",");
                    let setup = G::Args::FIELDS.iter().enumerate().filter(|(_, (_, kind))| *kind == crate::ArgumentKind::Setup).map(|(i, _)| i.to_string()).collect::<Vec<_>>().join(",");
                    return Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"reload\":{{\"rebased\":true,\"releasedInput\":{},\"values\":[{values}],\"setupIndices\":[{setup}]}},\"ownership\":{}}}", w.hash(), q.release_input, self.ownership_json()));
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
    fn layout_json(&self, e: Entity, viewport: Vec2) -> Result<String, String> {
        let w = &self.world;
        let layout = spatial::layout(w, viewport, e);
        let (scale, rotation, position) = layout.pose.to_scale_rotation_translation();
        let corners = layout.corners;
        let lo = corners
            .iter()
            .copied()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let hi = corners
            .iter()
            .copied()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        let (screen, depth, visible) =
            if let Some((inside, behind, distance, depth)) = layout.visibility {
                let screen = layout
                    .screen
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
                (
                    screen,
                    encode(&depth)?,
                    format!(
                        "{{\"inFrustum\":{},\"behindCamera\":{behind},\"distance\":{}}}",
                        inside && w.get::<Visible>(e).is_none_or(|v| v.0),
                        encode(&distance)?
                    ),
                )
            } else {
                (
                    "{\"unavailable\":true}".into(),
                    "null".into(),
                    "{\"unavailable\":true}".into(),
                )
            };
        let bounds = if layout.unbounded {
            "null".into()
        } else {
            format!("{{\"min\":{},\"max\":{}}}", encode(&lo)?, encode(&hi)?)
        };
        let global = if w.global_position(e).is_some() {
            format!(
                "{{\"position\":{},\"rotation\":{},\"scale\":{}}}",
                json::to_string(&position).map_err(|e| e.to_string())?,
                encode(&rotation)?,
                encode(&scale)?
            )
        } else {
            "null".into()
        };
        Ok(format!("{{\"tick\":{},\"entity\":{{{},\"world\":{global},\"bounds\":{bounds},\"screen\":{screen},\"depth\":{depth},\"visible\":{visible}}}}}", w.tick(),identity(w,e)))
    }
}
