use crate::{Data, DataError, Reader, Writer};
use std::{any::Any, collections::BTreeMap, sync::Arc};

/// The asset carrying one complete tuning transaction.
pub const TABLE_ASSET: &str = "exact-tables.json";

/// A named tuning record, checked by the same decoder at bake and delivery.
#[derive(Clone, Copy)]
pub struct Table {
    /// Authored JSON path, relative to the game directory.
    pub name: &'static str,
    decode: fn(&str) -> Result<TableValue, DataError>,
}
impl Table {
    /// Declare a tuning record without a separate schema.
    pub const fn of<T: Data + Send + Sync>(name: &'static str) -> Self {
        Self {
            name,
            decode: decode::<T>,
        }
    }
    /// Validate a complete record, including required fields.
    pub fn decode(self, text: &str) -> Result<TableValue, DataError> {
        if !super::asset_name(self.name)
            || !self.name.ends_with(".json")
            || self.name.ends_with(".level.json")
        {
            return Err(DataError::new("expected a tuning .json name, not a level").at(self.name));
        }
        (self.decode)(text).map_err(|e| e.at(self.name))
    }
}
fn decode<T: Data + Send + Sync>(text: &str) -> Result<TableValue, DataError> {
    let value: T = crate::json::from_complete_str(text)?;
    Ok(TableValue {
        value: Arc::new(value),
        ty: std::any::type_name::<T>(),
    })
}
/// A decoded immutable version. Game reads borrow this allocation.
#[derive(Clone)]
pub struct TableValue {
    value: Arc<dyn Any + Send + Sync>,
    ty: &'static str,
}

/// Piecewise linear points with strictly increasing finite x coordinates.
#[derive(Default, Clone, Debug)]
pub struct Curve(pub Vec<(f32, f32)>);
impl Curve {
    /// Sample the line; outside its domain, keep the nearest endpoint.
    pub fn at(&self, x: f32) -> f32 {
        let Some(&(first, value)) = self.0.first() else {
            return 0.0;
        };
        if x <= first {
            return value;
        }
        for pair in self.0.windows(2) {
            let [(a, av), (b, bv)] = pair else {
                unreachable!()
            };
            if x <= *b {
                return av + (bv - av) * ((x - a) / (b - a));
            }
        }
        self.0.last().unwrap().1
    }
}
impl Data for Curve {
    fn write(&self, w: &mut dyn Writer) {
        self.0.write(w);
    }
    fn read(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let mut points = Vec::<(f32, f32)>::new();
        points.read(r)?;
        for (i, &(x, y)) in points.iter().enumerate() {
            if !x.is_finite() || !y.is_finite() || (i > 0 && points[i - 1].0 >= x) {
                return Err(DataError::new(
                    "curve points must be finite and x strictly increasing",
                )
                .at(i));
            }
        }
        self.0 = points;
        Ok(())
    }
}

/// Complete table bytes admitted for one completed tick; replay retains the bytes.
#[derive(Default, Clone, Data)]
pub struct TableInput {
    /// Tick at whose start this transaction becomes visible (zero is setup).
    pub tick: u64,
    /// Each declared path and its complete JSON record.
    pub rows: BTreeMap<String, String>,
}
/// Active values are simulation resources, covered by hashes and inspection.
#[derive(Default, crate::Resource)]
pub(crate) struct TableValues(pub BTreeMap<String, String>);
#[derive(Default, Clone, Data)]
struct History {
    active: BTreeMap<String, String>,
    inputs: Vec<TableInput>,
    pending: Vec<TableInput>,
}
#[derive(Clone)]
pub(crate) struct TableStore {
    declarations: &'static [Table],
    history: History,
    active: BTreeMap<String, TableValue>,
    versions: BTreeMap<String, BTreeMap<u64, TableValue>>,
}
impl TableStore {
    fn new(declarations: &'static [Table]) -> Self {
        Self {
            declarations,
            history: History::default(),
            active: BTreeMap::new(),
            versions: BTreeMap::new(),
        }
    }
    fn activate(&mut self, rows: BTreeMap<String, String>) {
        self.active = rows
            .iter()
            .map(|(name, text)| {
                (
                    name.clone(),
                    self.versions[name][&crate::hash::of(text)].clone(),
                )
            })
            .collect();
        self.history.active = rows;
    }
    fn validate(&mut self, rows: &BTreeMap<String, String>) -> Result<(), DataError> {
        if rows.len() != self.declarations.len() {
            return Err(DataError::new(
                "transaction must contain every declared table",
            ));
        }
        for table in self.declarations {
            let text = rows
                .get(table.name)
                .ok_or_else(|| DataError::new("missing table").at(table.name))?;
            let versions = self.versions.entry(table.name.into()).or_default();
            let digest = crate::hash::of(text);
            if let std::collections::btree_map::Entry::Vacant(slot) = versions.entry(digest) {
                slot.insert(table.decode(text)?);
            }
        }
        Ok(())
    }
}

/// Validate a transaction using the game's declaration, without a world or a compiler.
pub fn validate_tables<G: crate::Game>(text: &str) -> Result<(), DataError> {
    let rows: BTreeMap<String, String> = crate::json::from_str(text)?;
    TableStore::new(G::TABLES).validate(&rows)
}

impl crate::World {
    /// Borrow a typed version, decoded once at admission and retained for this session.
    pub fn table<T: Data>(&self, name: &str) -> Result<&T, DataError> {
        let store = self
            .assets
            .tables
            .as_ref()
            .ok_or_else(|| DataError::new("tables have not arrived").at(name))?;
        let value = store
            .active
            .get(name)
            .ok_or_else(|| DataError::new("table has not arrived").at(name))?;
        value
            .value
            .downcast_ref()
            .ok_or_else(|| DataError::new(format!("declared type is `{}`", value.ty)).at(name))
    }
    pub(crate) fn redecode_tables(&mut self) {
        if let Some(old) = &self.assets.tables {
            let mut fresh = TableStore::new(old.declarations);
            for input in old.history.inputs.iter().chain(&old.history.pending) {
                fresh.validate(&input.rows).expect("retained table input");
            }
            fresh.activate(old.history.active.clone());
            fresh.history = old.history.clone();
            self.assets.tables = Some(Arc::new(fresh));
        }
    }
    pub(crate) fn table_pending_tick(&self) -> Option<u64> {
        self.assets
            .tables
            .as_ref()?
            .history
            .pending
            .first()
            .map(|input| input.tick)
    }
    pub(crate) fn activate_tables(&mut self) {
        let Some(current) = self.assets.tables.as_ref() else {
            return;
        };
        let tick = self.tick() + 1;
        if !current
            .history
            .pending
            .first()
            .is_some_and(|input| input.tick <= tick)
        {
            return;
        }
        let mut store = (**current).clone();
        while store
            .history
            .pending
            .first()
            .is_some_and(|input| input.tick <= tick)
        {
            let input = store.history.pending.remove(0);
            self.log(format_args!(
                "table swap at tick {}: {}; setup-only reads take effect at the next setup",
                input.tick,
                input.rows.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
            store.activate(input.rows.clone());
            store.history.inputs.push(input);
        }
        self.insert_resource(TableValues(store.history.active.clone()));
        self.assets.tables = Some(Arc::new(store));
    }
    pub(crate) fn setup_tables(&mut self) {
        if let Some(current) = &self.assets.tables {
            let mut store = (**current).clone();
            store.history.inputs = vec![TableInput {
                tick: 0,
                rows: store.history.active.clone(),
            }];
            store.history.pending.clear();
            self.insert_resource(TableValues(store.history.active.clone()));
            self.assets.tables = Some(Arc::new(store));
        }
    }
    pub(crate) fn validate_tables_restore(&self, seen: bool) -> Result<(), DataError> {
        if let Some(store) = &self.assets.tables {
            if !seen {
                return Err(DataError::new("restore refused: table inputs are missing"));
            }
            if self
                .try_resource::<TableValues>()
                .is_none_or(|values| values.0 != store.history.active)
            {
                return Err(DataError::new(
                    "restore refused: table resources disagree with retained inputs",
                ));
            }
            if store.history.inputs.first().is_none_or(|i| i.tick != 0)
                || store
                    .history
                    .inputs
                    .last()
                    .is_none_or(|i| i.rows != store.history.active)
            {
                return Err(DataError::new(
                    "restore refused: active tables disagree with input history",
                ));
            }
            let mut last = 0;
            for input in &store.history.inputs {
                if input.tick < last || input.tick > self.tick() {
                    return Err(DataError::new("restore refused: invalid table input tick"));
                }
                last = input.tick;
            }
            for input in &store.history.pending {
                if input.tick <= self.tick() || input.tick < last {
                    return Err(DataError::new(
                        "restore refused: invalid pending table tick",
                    ));
                }
                last = input.tick;
            }
        }
        Ok(())
    }
    pub(crate) fn write_tables(&self, w: &mut dyn Writer) {
        if let Some(store) = &self.assets.tables {
            w.field("tableInputs");
            store.history.write(w);
        }
    }
    pub(crate) fn read_tables(&mut self, r: &mut dyn Reader) -> Result<(), DataError> {
        let current = self
            .assets
            .tables
            .as_ref()
            .ok_or_else(|| DataError::new("restore refused: tables were not delivered"))?;
        let mut history = History::default();
        history.read(r)?;
        for table in current.declarations {
            if history.active.get(table.name) != current.history.active.get(table.name) {
                return Err(
                    DataError::new("restore refused: table identity differs").at(table.name)
                );
            }
        }
        let mut store = (**current).clone();
        store.validate(&history.active)?;
        for input in history.inputs.iter().chain(&history.pending) {
            store.validate(&input.rows)?;
        }
        store.activate(history.active.clone());
        store.history = history;
        self.assets.tables = Some(Arc::new(store));
        Ok(())
    }
}

impl<G: crate::Game> crate::Sim<G> {
    pub(crate) fn deliver_tables(&mut self, text: &str) -> Result<(), String> {
        let result = (|| -> Result<(), DataError> {
            if G::TABLES.is_empty() {
                return Err(DataError::new("no Game::TABLES declared"));
            }
            let rows: BTreeMap<String, String> = crate::json::from_str(text)?;
            let old = self.world.assets.tables.as_ref();
            let mut store = old.map_or_else(|| TableStore::new(G::TABLES), |old| (**old).clone());
            store.validate(&rows)?;
            if old.is_none() {
                store.activate(rows.clone());
                store.history.inputs.push(TableInput { tick: 0, rows });
                self.world
                    .insert_resource(TableValues(store.history.active.clone()));
            } else if store
                .history
                .pending
                .last()
                .map_or(&store.history.active, |input| &input.rows)
                != &rows
            {
                // TODO(1127.004 D6): use the shared clock-stamped admission contract.
                // Until it lands, a complete transaction activates at the next tick boundary.
                store.history.pending.push(TableInput {
                    tick: self.world.tick() + 1,
                    rows,
                });
            }
            self.world.assets.tables = Some(Arc::new(store));
            self.world
                .assets
                .states
                .insert(TABLE_ASSET.into(), super::AssetState::Loaded);
            Ok(())
        })();
        if let Err(error) = &result {
            self.world.log(format_args!("table swap refused: {error}"));
        }
        result.map_err(|e| e.to_string())
    }
    /// Development transactions admitted after setup, including those awaiting a tick.
    pub fn table_edits(&self) -> usize {
        self.world.assets.tables.as_ref().map_or(0, |store| {
            store.history.inputs.len().saturating_sub(1) + store.history.pending.len()
        })
    }
    /// Recorded table inputs, including setup bytes, for a deterministic replay.
    pub fn table_inputs(&self) -> Vec<TableInput> {
        self.world
            .assets
            .tables
            .as_ref()
            .map_or_else(Vec::new, |s| s.history.inputs.clone())
    }
    /// Replay retained transactions on their original ticks after delivering the setup version.
    pub fn replay_tables(&mut self, inputs: &[TableInput]) -> Result<(), DataError> {
        let old = self
            .world
            .assets
            .tables
            .as_ref()
            .ok_or_else(|| DataError::new("deliver setup tables before replay"))?;
        if self.world.tick() != 0
            || inputs
                .first()
                .is_none_or(|i| i.tick != 0 || i.rows != old.history.active)
        {
            return Err(DataError::new(
                "table replay requires the original setup version at tick zero",
            ));
        }
        let mut store = (**old).clone();
        let mut last = 0;
        for input in &inputs[1..] {
            if input.tick == 0 || input.tick < last {
                return Err(DataError::new("table replay ticks are not ordered"));
            }
            store.validate(&input.rows)?;
            last = input.tick;
        }
        store.history.pending = inputs[1..].to_vec();
        self.world.assets.tables = Some(Arc::new(store));
        Ok(())
    }
}
