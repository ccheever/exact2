//! Tick-zero authored values, merged only at the atomic Sim restore boundary.
use super::*;
#[path = "reload_value.rs"]
mod value;
use value::{Collect, Node, Work};

const MAX_ENTITIES: usize = 1_000_000;
const MAX_BASE_BYTES: usize = 128 * 1024 * 1024;
#[derive(Default, Data)]
struct Row {
    entity: Entity,
    components: BTreeMap<String, Node>,
}
#[derive(Default, Data)]
struct Initial {
    entities: BTreeMap<String, Row>,
    resources: BTreeMap<String, Node>,
}

pub(super) fn key(world: &World, e: Entity) -> String {
    if let Some(name) = world.name(e) {
        if world.names.get(name).is_some_and(|es| es.len() == 1) {
            return format!("n:{name}");
        }
        return format!("ambiguous:{}", e.index());
    }
    if world.contains(e) {
        format!("u:{}", e.index())
    } else {
        format!("stale:{}:{}", e.index(), e.generation())
    }
}
pub(super) fn resolve(world: &World, key: &str) -> Option<Entity> {
    if let Some(name) = key.strip_prefix("n:") {
        return world
            .names
            .get(name)
            .filter(|es| es.len() == 1)?
            .first()
            .copied();
    }
    if let Some(reference) = key.strip_prefix("r:") {
        let (index, generation) = reference.split_once(':')?;
        let e = Entity {
            index: index.parse().ok()?,
            generation: generation.parse().ok()?,
        };
        return world.contains(e).then_some(e);
    }
    let i = key.strip_prefix("u:")?.parse::<usize>().ok()?;
    let slot = world.state.slots.get(i)?;
    (slot.alive && slot.name.is_none()).then(|| world.entity_at(i))
}
fn label(key: &str) -> String {
    key.strip_prefix("n:")
        .map(str::to_owned)
        .unwrap_or_else(|| format!("#{}", key.strip_prefix("u:").unwrap_or(key)))
}
impl World {
    pub(crate) fn initializer(&self) -> Result<Vec<u8>, DataError> {
        if self.state.slots.len() > MAX_ENTITIES
            || self.components.len() + self.resources.len() > 256
        {
            return Err(DataError::new(
                "reload initializer exceeds 1000000 slots / 256 storage types",
            ));
        }
        let mut work = Work::default();
        // Keep only one component's projection alive while writing the compact
        // base. Sorting identity keys preserves the same deterministic map order.
        let mut entities: Vec<_> = self.entities().map(|e| (key(self, e), e)).collect();
        entities.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut out = bin::Encoder::default();
        out.begin_struct();
        out.field("entities");
        out.begin_struct();
        for (name, entity) in entities {
            work.charge(128 + name.len());
            work.check()?;
            out.key(&name);
            out.begin_struct();
            out.field("entity");
            entity.write(&mut out);
            out.field("components");
            out.begin_struct();
            for (name, storage) in &self.components {
                work.charge(0);
                if storage.has(entity.index as usize) {
                    let mut w = Collect::new(self, &mut work);
                    storage.write_one(entity.index as usize, &mut w);
                    out.key(name);
                    w.finish()?.write(&mut out);
                }
            }
            out.end_struct();
            out.end_struct();
        }
        out.end_struct();
        out.field("resources");
        out.begin_struct();
        for (name, storage) in &self.resources {
            let mut w = Collect::new(self, &mut work);
            storage.write_one(0, &mut w);
            out.key(name);
            w.finish()?.write(&mut out);
        }
        out.end_struct();
        out.end_struct();
        work.check()?;
        let bytes = out.finish();
        if bytes.len() > MAX_BASE_BYTES {
            return Err(DataError::new("reload initializer exceeds 128 MiB"));
        }
        Ok(bytes)
    }
    pub(crate) fn merge_initializer(
        &self,
        base: &[u8],
        theirs: &[u8],
        budget: Option<&crate::data::LoadBudget>,
    ) -> Result<Report, DataError> {
        if base == theirs {
            return Ok(Report::default());
        }
        if base.len().max(theirs.len()) > MAX_BASE_BYTES {
            return Err(DataError::new("reload initializer exceeds 128 MiB"));
        }
        let allowance = crate::data::LoadBudget::new(1024 * 1024 * 1024);
        let budget = Some(budget.unwrap_or(&allowance));
        let base: Initial = bin::from_slice_in(base, budget)?;
        let theirs: Initial = bin::from_slice_in(theirs, budget)?;
        if base.entities.len().max(theirs.entities.len()) > MAX_ENTITIES {
            return Err(DataError::new("reload initializer exceeds 1000000 slots"));
        }
        let mut report = Report::default();
        let mut work = Work::default();
        let matched: BTreeSet<_> = base
            .entities
            .iter()
            .filter_map(|(k, b)| {
                let t = theirs.entities.get(k)?;
                let mine = resolve(self, k)?;
                (k.starts_with("n:")
                    || (k.starts_with("u:")
                        && mine == b.entity
                        && b.components.keys().eq(t.components.keys())))
                .then_some(k.as_str())
            })
            .collect();
        for k in base
            .entities
            .keys()
            .chain(theirs.entities.keys())
            .collect::<BTreeSet<_>>()
        {
            let b = base.entities.get(k);
            let t = theirs.entities.get(k);
            let entity = label(k);
            match (b, t) {
                (Some(b), Some(t)) if matched.contains(k.as_str()) => {
                    let e = resolve(self, k).unwrap();
                    self.merge_components(
                        &entity,
                        e.index as usize,
                        &b.components,
                        &t.components,
                        &self.components,
                        &matched,
                        &mut work,
                        &mut report,
                    )?;
                }
                (None, Some(_)) if k.starts_with("n:") => report.push(
                    "added",
                    &entity,
                    "*",
                    "*",
                    "absent",
                    "present",
                    "restart to instantiate",
                ),
                (Some(_), None) if k.starts_with("n:") => report.push(
                    "removed",
                    &entity,
                    "*",
                    "*",
                    "present",
                    "absent",
                    "restart to remove",
                ),
                _ => report.push(
                    "unmatched",
                    &entity,
                    "*",
                    "*",
                    "base",
                    "new initializer",
                    "identity or component set differs; restart",
                ),
            }
        }
        self.merge_components(
            "resource",
            0,
            &base.resources,
            &theirs.resources,
            &self.resources,
            &matched,
            &mut work,
            &mut report,
        )?;
        Ok(report)
    }
    #[allow(clippy::too_many_arguments)]
    fn merge_components(
        &self,
        entity: &str,
        index: usize,
        base: &BTreeMap<String, Node>,
        theirs: &BTreeMap<String, Node>,
        storage: &BTreeMap<&str, Box<dyn Erased>>,
        matched: &BTreeSet<&str>,
        work: &mut Work,
        report: &mut Report,
    ) -> Result<(), DataError> {
        for name in base.keys().chain(theirs.keys()).collect::<BTreeSet<_>>() {
            work.charge(0);
            work.check()?;
            match (base.get(name), theirs.get(name)) {
                (Some(b), Some(t)) if b != t => {
                    let Some(s) = storage.get(name.as_str()).filter(|s| s.has(index)) else {
                        report.push(
                            "unmatched",
                            entity,
                            name,
                            "*",
                            "present",
                            "absent",
                            "carried component missing; restart",
                        );
                        continue;
                    };
                    let mut w = Collect::new(self, work);
                    s.write_one(index, &mut w);
                    let mine = w.finish()?;
                    let mut merger = Merge {
                        world: self,
                        entity,
                        component: name,
                        report,
                        matched,
                        work,
                    };
                    if let Some(patch) = merger.field("", b, t, &mine)? {
                        let mut w = bin::Encoder::default();
                        patch.emit(&mut w, self)?;
                        let bytes = w.finish();
                        let mut r = bin::Decoder::patch(&bytes);
                        s.patch(index, &mut r)?;
                        r.finish()?;
                    }
                }
                (None, Some(_)) => report.push(
                    "added",
                    entity,
                    name,
                    "*",
                    "absent",
                    "present",
                    "restart to instantiate",
                ),
                (Some(_), None) => report.push(
                    "removed",
                    entity,
                    name,
                    "*",
                    "present",
                    "absent",
                    "restart to remove",
                ),
                _ => {}
            }
        }
        Ok(())
    }
}
struct Merge<'a> {
    world: &'a World,
    entity: &'a str,
    component: &'a str,
    report: &'a mut Report,
    matched: &'a BTreeSet<&'a str>,
    work: &'a mut Work,
}
impl Merge<'_> {
    fn field(
        &mut self,
        path: &str,
        base: &Node,
        theirs: &Node,
        mine: &Node,
    ) -> Result<Option<Node>, DataError> {
        self.work.charge(0);
        self.work.check()?;
        if base == theirs {
            return Ok(None);
        }
        if let (Node::Record(b), Node::Record(t), Node::Record(m)) = (base, theirs, mine) {
            let mut patch = BTreeMap::new();
            for name in b.keys().chain(t.keys()).collect::<BTreeSet<_>>() {
                let path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                match (b.get(name), t.get(name), m.get(name)) {
                    (Some(b), Some(t), Some(m)) => {
                        if let Some(v) = self.field(&path, b, t, m)? {
                            patch.insert(name.clone(), v);
                        }
                    }
                    (None, Some(t), _) => self.report.push(
                        "added",
                        self.entity,
                        self.component,
                        &path,
                        "absent",
                        &t.display(self.world),
                        "restart to initialize new field",
                    ),
                    (Some(b), None, _) => self.report.push(
                        "removed",
                        self.entity,
                        self.component,
                        &path,
                        &b.display(self.world),
                        "absent",
                        "restart to remove field",
                    ),
                    _ => self.report.push(
                        "unmatched",
                        self.entity,
                        self.component,
                        &path,
                        "present",
                        "absent",
                        "carried field missing; restart",
                    ),
                }
            }
            return Ok((!patch.is_empty()).then_some(Node::Record(patch)));
        }
        if let (Node::Variant(bn, _, b), Node::Variant(tn, ti, t), Node::Variant(mn, _, m)) =
            (base, theirs, mine)
        {
            if bn == tn && bn == mn {
                return Ok(self
                    .field(path, b, t, m)?
                    .map(|v| Node::Variant(tn.clone(), *ti, Box::new(v))));
            }
        }
        if let (Node::Option(true, b), Node::Option(true, t), Node::Option(true, m)) =
            (base, theirs, mine)
        {
            return Ok(self
                .field(path, b, t, m)?
                .map(|v| Node::Option(true, Box::new(v))));
        }
        // Containers are one field, as in Data save/load. Record children merge by name.
        let old = base.display(self.world);
        let new = theirs.display(self.world);
        if base != mine {
            self.report.push(
                "kept",
                self.entity,
                self.component,
                path,
                &old,
                &new,
                "initializer changed, applies on restart",
            );
            return Ok(None);
        }
        if !references_match(theirs, self.matched) || {
            let mut encoder = bin::Encoder::default();
            theirs.emit(&mut encoder, self.world).is_err()
        } {
            self.report.push(
                "unmatched",
                self.entity,
                self.component,
                path,
                &old,
                &new,
                "entity reference needs restart",
            );
            return Ok(None);
        }
        self.report.push(
            "applied",
            self.entity,
            self.component,
            path,
            &old,
            &new,
            "authored initializer changed",
        );
        Ok(Some(theirs.clone()))
    }
}
fn references_match(node: &Node, matched: &BTreeSet<&str>) -> bool {
    match node {
        Node::Reference(k) => {
            k == "null"
                || k.starts_with("n:")
                || k.strip_prefix("r:")
                    .and_then(|s| s.split_once(':'))
                    .is_some_and(|(index, _)| matched.contains(format!("u:{index}").as_str()))
        }
        Node::Record(v) => v.values().all(|v| references_match(v, matched)),
        Node::Seq(v) => v.iter().all(|v| references_match(v, matched)),
        Node::Variant(_, _, v) | Node::Option(true, v) => references_match(v, matched),
        _ => true,
    }
}

#[derive(Clone, Default, Data)]
struct Item {
    entity: String,
    component: String,
    field: String,
    old: String,
    new: String,
    reason: String,
}
#[derive(Clone, Default, Data)]
pub(crate) struct Report {
    applied: Vec<Item>,
    kept: Vec<Item>,
    added: Vec<Item>,
    removed: Vec<Item>,
    unmatched: Vec<Item>,
    omitted: u64,
}
impl Report {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        kind: &str,
        entity: &str,
        component: &str,
        field: &str,
        old: &str,
        new: &str,
        reason: &str,
    ) {
        if self.applied.len()
            + self.kept.len()
            + self.added.len()
            + self.removed.len()
            + self.unmatched.len()
            == 64
        {
            self.omitted += 1;
            return;
        }
        let list = match kind {
            "applied" => &mut self.applied,
            "kept" => &mut self.kept,
            "added" => &mut self.added,
            "removed" => &mut self.removed,
            _ => &mut self.unmatched,
        };
        let bounded = |s: &str| {
            if s.chars().count() > 256 {
                format!("{}…", s.chars().take(255).collect::<String>())
            } else {
                s.into()
            }
        };
        list.push(Item {
            entity: bounded(entity),
            component: bounded(component),
            field: bounded(field),
            old: bounded(old),
            new: bounded(new),
            reason: reason.into(),
        });
    }
    pub(crate) fn json(&self) -> String {
        crate::json::to_string(self).expect("reload report is bounded text")
    }
    pub(crate) fn log(&self, world: &World) {
        for (kind, items) in [
            ("applied", &self.applied),
            ("kept", &self.kept),
            ("added", &self.added),
            ("removed", &self.removed),
            ("unmatched", &self.unmatched),
        ] {
            for item in items {
                world.session_log(format_args!(
                    "reload {kind}: {}.{}.{} {} → {} ({})",
                    item.entity, item.component, item.field, item.old, item.new, item.reason
                ));
            }
        }
        if self.omitted > 0 {
            world.session_log(format_args!(
                "reload: {} additional items omitted",
                self.omitted
            ));
        }
    }
}
#[cfg(test)]
mod tests;
