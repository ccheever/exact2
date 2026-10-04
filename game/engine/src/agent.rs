use crate::values::quote;
use crate::{
    json, spatial, Clock, Data, DataError, Entity, Game, Parent, Reader, Sim, Vec2, Vec3, Visible,
    World,
};
use std::collections::BTreeMap;
use std::fmt::Write;

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
    width: Option<f32>,
    height: Option<f32>,
    x: Option<f32>,
    y: Option<f32>,
    since: u64,
    from: u64,
    limit: Option<u64>,
    resources: bool,
}
impl Request {
    fn parse(text: &str) -> Result<Self, DataError> {
        let mut r = json::Decoder::new(text);
        let mut q = Self::default();
        r.begin_struct()?;
        while let Some(f) = r.field()? {
            match f.as_str() {
                "op" => q.op.read(&mut r)?,
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
                "from" => q.from.read(&mut r)?,
                "limit" => {
                    let mut n = 0u64;
                    n.read(&mut r)?;
                    if n == 0 || n > MAX_PAGE as u64 {
                        return Err(DataError::new(format!(
                            "limit must be 1..={MAX_PAGE}; page with from"
                        ))
                        .at("limit"));
                    }
                    q.limit = Some(n);
                }
                "resources" => q.resources.read(&mut r)?,
                _ => r.skip()?,
            }
        }
        r.finish()?;
        Ok(q)
    }
}
// Entity listings answer in pages: `from` an offset in hierarchy preorder, at most
// `limit` entities (512 by default), with `total` and the `next` offset.
const PAGE: usize = 512;
const MAX_PAGE: usize = 100_000;
fn page(q: &Request, start: usize, end: usize) -> (usize, usize) {
    let from = start
        .saturating_add(q.from.min(usize::MAX as u64) as usize)
        .min(end);
    let limit = q.limit.map_or(PAGE, |n| n as usize);
    (from, from.saturating_add(limit).min(end))
}
fn page_tail(out: &mut String, to: usize, start: usize, end: usize) {
    write!(out, "\"truncated\":{},\"total\":{}", to < end, end - start).unwrap();
    if to < end {
        write!(out, ",\"next\":{}", to - start).unwrap();
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
fn identity_into(out: &mut String, w: &World, e: Entity) {
    write!(out, "\"id\":{},\"name\":", e.index()).unwrap();
    if let Some(name) = w.name(e) {
        json::quote_into(out, name);
    } else {
        out.push_str("null");
    }
}
fn hierarchy(w: &World) -> Result<Vec<(Entity, Option<Entity>, u32)>, String> {
    let mut parents = w.query::<Option<&Parent>>();
    let mut children: BTreeMap<Option<Entity>, Vec<Entity>> = BTreeMap::new();
    for (e, parent) in parents.iter() {
        children
            .entry(parent.map(|p| p.0).filter(|p| w.contains(*p)))
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
            .filter_map(|(e, p)| p.map(|p| (e, p.0)))
            .filter(|(e, p)| w.contains(*p) && !out.iter().any(|(seen, _, _)| seen == e))
            .map(|(e, p)| format!("{} -> Parent #{}", identity(w, e), p.index()))
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
        self.world.assets.states.values().filter_map(|s| match s {
            crate::asset::AssetState::Failed(reason) => Some(reason.as_str()),
            _ => None,
        })
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
        if q.width.is_some() != q.height.is_some() {
            return Err("width and height must be supplied together".into());
        }
        if let (Some(w), Some(h)) = (q.width, q.height) {
            self.viewport(w, h);
        }
        if let Some(now) = q.now {
            self.advance_with(now, Clock::Seekable, after);
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
                let (from, to) = page(&q, start, end);
                let mut out = format!("{{\"tick\":{tick},\"entities\":[");
                for (i, &(e, parent, depth)) in all[from..to].iter().enumerate() {
                    if i != 0 { out.push(','); }
                    out.push('{');
                    identity_into(&mut out, w, e);
                    out.push_str(",\"parent\":");
                    if let Some(parent) = parent { write!(out, "{}", parent.index()).unwrap(); } else { out.push_str("null"); }
                    write!(out, ",\"depth\":{depth},\"components\":[").unwrap();
                    for (i, name) in w.component_names(e).enumerate() {
                        if i != 0 { out.push(','); }
                        json::quote_into(&mut out, name);
                    }
                    out.push_str("],\"tags\":[]}");
                }
                out.push_str("],");
                page_tail(&mut out, to, start, end);
                out.push('}');
                Ok(out)
            }
            "state" if q.entity.as_deref() == Some("*") => {
                let all = hierarchy(w)?;
                let subtree = q.under.as_deref().map(|n| resolve(w,n)).transpose()?;
                let start = subtree.and_then(|e| all.iter().position(|(a,_,_)| *a == e)).unwrap_or(0);
                let end = if subtree.is_some() { (start+1..all.len()).find(|&i| all[i].2 <= all[start].2).unwrap_or(all.len()) } else { all.len() };
                let (from, to) = page(&q, start, end);
                let mut entities = String::new();
                for (i, &(e, _, _)) in all[from..to].iter().enumerate() {
                    if i != 0 { entities.push(','); }
                    entities.push('{');
                    identity_into(&mut entities, w, e);
                    entities.push_str(",\"components\":");
                    entities.push_str(&w.components_json(e).map_err(|e| e.to_string())?);
                    entities.push('}');
                }
                let mut tail = String::new();
                page_tail(&mut tail, to, start, end);
                let resources = if q.resources { format!(",\"resources\":{}", w.resources_json().map_err(|e| e.to_string())?) } else { String::new() };
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"entities\":[{entities}],{tail}{resources}{}}}", w.hash(), if q.busy { format!(",\"busy\":{}", encode(&self.changing(self.quiescent()))?) } else { String::new() }))
            }
            "state" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().unwrap())?;
                if q.pose { return Ok(format!("{{\"tick\":{tick},\"entity\":{{{}}},\"pose\":{}}}",identity(w,e),inspect(w,e,true)?)); }
                Ok(format!("{{\"tick\":{tick},\"entity\":{{{},\"components\":{}{}}}}}", identity(w,e), w.components_json(e).map_err(|e|e.to_string())?, inspect(w,e,false)?))
            }
            "state" => {
                let host_input = self.host_input();
                Ok(format!("{{\"tick\":{tick},\"world\":{{\"name\":{},\"tick\":{tick},\"hz\":{},\"seed\":{},\"hash\":\"0x{:016x}\",\"entities\":{},\"paused\":{},\"loading\":{},\"assets\":{},\"restarted\":{},\"restored\":{}{}{},\"args\":{},\"resources\":{},\"audio\":{},\"input\":{{\"actions\":{},\"held\":{},\"forwarded\":{},\"controls\":{},\"forwardedControls\":{},\"controlContacts\":{},\"refusedPosts\":{}}},\"published\":{}}}}}",
                quote(G::NAME), w.hz(), w.seed(), w.hash(), w.len(), G::paused(&self.args), encode(&w.assets.states.iter().filter(|(_, s)| **s == crate::asset::AssetState::Pending).map(|(n, _)| n.clone()).collect::<Vec<_>>())?, w.assets.state_json(), self.restarted, self.restored, self.restored_from.as_ref().filter(|_| self.restored).map_or_else(String::new, |a| format!(",\"restoredFrom\":{a}")), self.paranoid_samples().map_or_else(String::new, |(skipped, owed)| format!(",\"paranoid\":{{\"skipped\":{skipped},\"owed\":{owed}}}")), self.args_json, w.resources_json().map_err(|e|e.to_string())?, crate::audio::state(w), self.input.actions().json(), encode(&self.input.keys)?, encode(&host_input.keys)?, encode(&self.input.held_controls())?, encode(&host_input.held_controls())?, encode(&host_input.control_contacts())?, self.refused_posts, w.published_json(true)))
            },
            "layout" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().ok_or("layout needs an entity")?)?;
                self.layout_json(e)
            }
            "layout" => {
                let point = Vec2::new(q.x.ok_or("layout needs x")?,q.y.ok_or("layout needs y")?);
                let view = spatial::View::new(w,self.input.viewport).ok_or("layout unavailable: needs an active camera and viewport; add `w.spawn_named(\"camera\", (Transform::at(0., 3., 8.), Camera::default()));` in setup; inspect `state world:*`")?;
                let hit = spatial::pick(w,&view,point).map(|(e,d,p)| Ok::<_,String>(format!("{{{},\"distance\":{},\"point\":{}}}", identity(w,e),encode(&d)?,encode(&p)?))).transpose()?.unwrap_or_else(||"null".into());
                Ok(format!("{{\"tick\":{tick},\"hit\":{hit}}}"))
            }
            "clock" if self.is_loading() => {
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":false,\"changing\":[\"loading\"],\"error\":{},\"assets\":{}}}", w.hash(), quote(&format!("clock refused: declared assets are not ready: {}; inspect untargeted `state`: world[0].loading and world[0].assets", w.assets.state_json())), w.assets.state_json()))
            }
            "clock" => {
                let quiescent = self.quiescent();
                let deadline = if quiescent { String::new() } else { format!(",\"settleAt\":{}", crate::data::text::Float(self.settle_at(q.settle))) };
                let changing = encode(&self.changing(quiescent))?;
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":{quiescent},\"changing\":{changing}{deadline}}}", w.hash()))
            },
            "logs" => Ok(w.journal_json(q.since)),
            _ => Err(format!("unknown op `{}`; use tree, screenshot, tap, type, state, layout, logs or clock",q.op)),
        }
    }
    fn layout_json(&self, e: Entity) -> Result<String, String> {
        let w = &self.world;
        let layout = spatial::layout(w, self.input.viewport, e);
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
