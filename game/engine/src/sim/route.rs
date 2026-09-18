use super::*;
use crate::{spatial, Entity, Vec3};

impl<G: Game> Sim<G> {
    /// Drive cardinal WASD input toward an XZ point, at the supplied metres/second.
    /// At most 160 bursts of ten ticks plus one released tick; no path planning.
    pub fn move_to(
        &mut self,
        entity: &str,
        target: Vec3,
        tolerance: f32,
        speed: f32,
    ) -> Result<(), String> {
        if !target.is_finite()
            || !tolerance.is_finite()
            || tolerance <= 0.0
            || !speed.is_finite()
            || speed <= 0.0
        {
            return Err(
                "move_to: finite target and positive finite tolerance/speed required".into(),
            );
        }
        let start = self
            .position(entity)
            .ok_or("move_to: position unavailable")?;
        let stride = speed / G::HZ as f32;
        for _ in 0..160 {
            let at = self
                .position(entity)
                .ok_or("move_to: position unavailable")?;
            let delta = target - at;
            if delta.x.hypot(delta.z) <= tolerance {
                return Ok(());
            }
            let x_axis = delta.x.abs() >= delta.z.abs();
            let gap = if x_axis { delta.x } else { delta.z };
            let key = match (x_axis, gap > 0.0) {
                (true, true) => "KeyD",
                (true, false) => "KeyA",
                (false, true) => "KeyS",
                (false, false) => "KeyW",
            };
            let count =
                (((gap.abs() - tolerance * 0.45).max(stride) / stride).floor() as u32).clamp(1, 10);
            self.key_down(key);
            let before = self.world().tick();
            let reply = self.agent(&format!(r#"{{"op":"clock","ticks":{count}}}"#));
            self.key_up(key);
            if self.world().tick() != before + count as u64 || reply.contains("\"error\"") {
                return Err(format!("move_to clock refused: {reply}"));
            }
            let reply = self.agent(r#"{"op":"clock","ticks":1}"#);
            if reply.contains("\"error\"") {
                return Err(format!("move_to clock refused: {reply}"));
            }
        }
        // Probe at head height for the 1.3m controller used by this input helper.
        let start = start + Vec3::Y * 0.65;
        let to = Vec3::new(target.x, start.y, target.z);
        let detail = self
            .route_diagnostic(entity, start, to)
            .unwrap_or_else(|e| format!("diagnostic unavailable: {e}"));
        Err(format!(
            "route stalled toward {},{} after 160 bursts: {detail}",
            target.x, target.z
        ))
    }

    /// Explain the straight segment using layout geometry. Sides are clear rays,
    /// not swept-character paths. Shares layout's slot and one-million-visit limits.
    pub fn route_diagnostic(&self, entity: &str, from: Vec3, to: Vec3) -> Result<String, String> {
        if !from.is_finite() || !to.is_finite() {
            return Err("route: finite endpoints required".into());
        }
        let subject = self
            .world
            .resolve(entity)
            .ok_or("route: entity unavailable")?;
        self.route_json(subject, None, from, to)
    }

    pub(crate) fn route_json(
        &self,
        subject: Entity,
        target: Option<Entity>,
        from: Vec3,
        to: Vec3,
    ) -> Result<String, String> {
        if !from.is_finite() || !to.is_finite() || !(to - from).length().is_finite() {
            return Err("route: endpoints exceed finite segment range".into());
        }
        let w = self.world();
        let mut sight = spatial::index::Sight::new(w, subject, target, None)?;
        let mut nearest: Option<(Entity, f32)> = None;
        sight.segment(from, to, 1, |e, distance| {
            if nearest
                .is_none_or(|(old, d)| distance < d || (distance == d && e.index() < old.index()))
            {
                nearest = Some((e, distance));
            }
            false
        })?;
        let Some((e, _)) = nearest else {
            return Ok(r#"{"blocker":null,"nearestClearSide":null}"#.into());
        };
        let index = e.index();
        let pose = w.global(e).ok_or("route: blocker pose unavailable")?;
        let mesh = w.get::<crate::Mesh>(e);
        let (half, center) = spatial::bounds(w, e, mesh.as_deref());
        let corners = spatial::corners(pose, half, center);
        let lo = corners
            .into_iter()
            .fold(Vec3::splat(f32::INFINITY), Vec3::min);
        let hi = corners
            .into_iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
        // Four side approaches, clamped outside the box, at the movement height.
        // A fixed 0.4m margin makes the answer useful for the consumer's 0.35m radius;
        // line-of-sight remains mesh geometry, not a collision/planning guarantee.
        let margin = 0.4;
        let candidates = [
            (
                "minX",
                Vec3::new(
                    lo.x - margin,
                    from.y,
                    from.z.clamp(lo.z - margin, hi.z + margin),
                ),
            ),
            (
                "maxX",
                Vec3::new(
                    hi.x + margin,
                    from.y,
                    from.z.clamp(lo.z - margin, hi.z + margin),
                ),
            ),
            (
                "minZ",
                Vec3::new(
                    from.x.clamp(lo.x - margin, hi.x + margin),
                    from.y,
                    lo.z - margin,
                ),
            ),
            (
                "maxZ",
                Vec3::new(
                    from.x.clamp(lo.x - margin, hi.x + margin),
                    from.y,
                    hi.z + margin,
                ),
            ),
        ];
        let mut clear = Vec::new();
        for (side, point) in candidates {
            if !sight.segment(from, point, 1, |_, _| true)? {
                clear.push((side, point));
            }
        }
        clear.sort_by(|a, b| {
            from.distance_squared(a.1)
                .total_cmp(&from.distance_squared(b.1))
        });
        let side = match clear.first() {
            Some((side, point)) => format!(
                r#"{{"side":"{side}","position":{}}}"#,
                crate::json::to_string(point).map_err(|e| e.to_string())?
            ),
            None => "null".into(),
        };
        let name = w
            .name(e)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{index}"));
        Ok(format!(
            r#"{{"blocker":{{"name":{},"position":{},"bounds":{{"min":{},"max":{}}}}},"nearestClearSide":{side}}}"#,
            crate::values::quote(&name),
            crate::json::to_string(&Vec3::from(pose.translation)).map_err(|e| e.to_string())?,
            crate::json::to_string(&lo).map_err(|e| e.to_string())?,
            crate::json::to_string(&hi).map_err(|e| e.to_string())?
        ))
    }
}
