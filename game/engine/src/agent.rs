use crate::values::quote;
use crate::{
    json, spatial, Clock, Data, DataError, Entity, Game, Mesh, Parent, Reader, Sim, Vec2, Vec3,
    Visible, World,
};
use std::collections::BTreeMap;

#[derive(Default)]
struct Request {
    op: String,
    entity: Option<String>,
    under: Option<String>,
    summary: bool,
    settle: bool,
    now: Option<f64>,
    width: Option<f32>,
    height: Option<f32>,
    x: Option<f32>,
    y: Option<f32>,
    since: u64,
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
        .ok_or_else(|| format!("no entity named `{name}`"))
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
        return Err("transform hierarchy contains a cycle".into());
    }
    Ok(out)
}
impl<G: Game> Sim<G> {
    /// Answer the engine half of an agent request as JSON, always tagged with tick.
    /// Component/resource Data uses [] for None and [value] for Some(value).
    pub fn agent(&mut self, request: &str) -> String {
        self.agent_with(request, |_, _| {})
    }
    /// Answer a request, observing the last ticks of any embedded clock advance.
    pub fn agent_with(&mut self, request: &str, after: impl FnMut(&World, u32)) -> String {
        match Request::parse(request)
            .map_err(|e| e.to_string())
            .and_then(|q| self.reply(q, after))
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
    fn reply(&mut self, q: Request, after: impl FnMut(&World, u32)) -> Result<String, String> {
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
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"entities\":[{entities}],\"truncated\":{}}}", w.hash(), end-start > 512))
            }
            "state" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().unwrap())?;
                Ok(format!("{{\"tick\":{tick},\"entity\":{{{},\"components\":{}}}}}", identity(w,e), w.components_json(e).map_err(|e|e.to_string())?))
            }
            "state" => Ok(format!("{{\"tick\":{tick},\"world\":{{\"name\":{},\"tick\":{tick},\"hz\":{},\"seed\":{},\"hash\":\"0x{:016x}\",\"entities\":{},\"paused\":{},\"loading\":{},\"assets\":{},\"restored\":{}{},\"args\":{},\"resources\":{},\"audio\":{},\"input\":{{\"actions\":{},\"held\":{},\"forwarded\":{}}},\"published\":{}}}}}",
                quote(G::NAME), w.hz(), w.seed(), w.hash(), w.len(), G::paused(&self.args), encode(&w.assets.states.iter().filter(|(_, s)| **s == crate::asset::AssetState::Pending).map(|(n, _)| n.clone()).collect::<Vec<_>>())?, w.assets.state_json(), self.restored, self.restored_from.as_ref().filter(|_| self.restored).map_or_else(String::new, |a| format!(",\"restoredFrom\":{a}")), crate::json::to_string(&self.args).map_err(|e| e.to_string())?, w.resources_json().map_err(|e|e.to_string())?, crate::audio::state(w), self.input.actions.json(), encode(&self.input.keys)?, encode(&self.held_keys())?, w.published_json(true))),
            "layout" if q.entity.is_some() => {
                let e = resolve(w,q.entity.as_deref().ok_or("layout needs an entity")?)?;
                self.layout_json(e)
            }
            "layout" => {
                let point = Vec2::new(q.x.ok_or("layout needs x")?,q.y.ok_or("layout needs y")?);
                let view = spatial::View::new(w,self.input.viewport).ok_or("layout unavailable: needs an active camera and viewport")?;
                let hit = spatial::pick(w,&view,point).map(|(e,d,p)| Ok::<_,String>(format!("{{{},\"distance\":{},\"point\":{}}}", identity(w,e),encode(&d)?,encode(&p)?))).transpose()?.unwrap_or_else(||"null".into());
                Ok(format!("{{\"tick\":{tick},\"hit\":{hit}}}"))
            }
            "clock" if self.is_loading() => {
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":false,\"changing\":[\"loading\"],\"error\":\"declared assets are not ready\",\"assets\":{}}}", w.hash(), w.assets.state_json()))
            }
            "clock" => {
                let quiescent = self.quiescent();
                let deadline = if quiescent { String::new() } else { format!(",\"settleAt\":{}", self.settle_at(q.settle)) };
                let changing = encode(&self.changing(quiescent))?;
                Ok(format!("{{\"tick\":{tick},\"hash\":\"0x{:016x}\",\"quiescent\":{quiescent},\"changing\":{changing}{deadline}}}", w.hash()))
            },
            "logs" => {
                let lines = w.journal(); let next = w.journal_next(); let start = lines.first().map_or(next, |e|e.index); let from = q.since.clamp(start,next);
                let lines = lines.iter().filter(|e|e.index >= from).map(|e|quote(&e.line)).collect::<Vec<_>>().join(",");
                Ok(format!("{{\"tick\":{tick},\"next\":{next},\"from\":{from},\"lines\":[{lines}]}}"))
            }
            _ => Err(format!("unknown op `{}`",q.op)),
        }
    }
    fn layout_json(&self, e: Entity) -> Result<String, String> {
        let w = &self.world;
        let pose = w.global(e).unwrap_or(crate::Affine3A::IDENTITY);
        let (scale, rotation, position) = pose.to_scale_rotation_translation();
        let mesh = w.get::<Mesh>(e);
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
        let (screen, depth, visible) =
            if let Some(view) = spatial::View::new(w, self.input.viewport) {
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
        let unbounded = matches!(mesh.as_deref(), Some(Mesh::Asset(name)) if w.model(name).is_none() && w.get::<crate::asset::ModelBounds>(e).is_none());
        let bounds = if unbounded {
            "null".into()
        } else {
            format!("{{\"min\":{},\"max\":{}}}", encode(&lo)?, encode(&hi)?)
        };
        Ok(format!("{{\"tick\":{},\"entity\":{{{},\"world\":{{\"position\":{},\"rotation\":{},\"scale\":{}}},\"bounds\":{bounds},\"screen\":{screen},\"depth\":{depth},\"visible\":{visible}}}}}", w.tick(),identity(w,e),encode(&position)?,encode(&rotation)?,encode(&scale)?))
    }
}
