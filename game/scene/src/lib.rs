//! Typed initial conditions over the existing World/Data authority (LLP 1041.006 §7).
//! Bake tools call `bake`; game setup only decodes the compiled artifact.
use exact_game::{
    bin, data::LoadBudget, Component, Data, DataError, Entity, Mesh, Parent, Reader, Resource,
    World,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub mod bake;
mod source;
const MAX_SCENE_BYTES: usize = 16 * 1024 * 1024;
const MAX_SCENE_ENTITIES: usize = 100_000;
// One cumulative decoder allowance, including opaque component payloads.
// Registered Rust checks and construction callbacks are trusted code, not a heap sandbox.
const MAX_SCENE_ALLOCATIONS: usize = 64 * 1024 * 1024;

/// Saved identity of the initial content; loading a world never reapplies that content.
#[derive(Default, Resource)]
pub struct SceneIdentity {
    pub digest: String,
    pub assets: BTreeMap<String, String>,
}
/// Procedural entities name their Rust generator and explicit inputs, never a fake line.
#[derive(Default, Component)]
pub struct GeneratedBy {
    pub generator: String,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Default, Data)]
struct Scene {
    entities: Vec<Row>,
    assets: BTreeMap<String, String>,
}
#[derive(Default, Data)]
struct Row {
    id: String,
    parent: String,
    components: BTreeMap<String, Vec<u8>>,
}
trait Insert {
    fn insert(self: Box<Self>, world: &mut World, entity: Entity);
}
impl<C: Component> Insert for C {
    fn insert(self: Box<Self>, world: &mut World, entity: Entity) {
        world.insert(entity, *self);
    }
}
type Prepared = Box<dyn Insert>;
type BakeValue = dyn Fn(&str) -> Result<Vec<u8>, DataError>;
type DecodeValue = dyn Fn(&[u8], &LoadBudget) -> Result<Prepared, DataError>;
struct Registration {
    bake: Box<BakeValue>,
    decode: Box<DecodeValue>,
    defaults: fn() -> Result<String, DataError>,
    register: fn(&mut World),
}
/// Component names select Rust constructors, not a separately maintained schema.
#[derive(Default)]
pub struct Types {
    types: BTreeMap<&'static str, Registration>,
}
impl Types {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn standard() -> Self {
        let mut types = Self::new();
        types
            .component::<exact_game::Transform>()
            .component::<exact_game::Material>()
            .component::<exact_game::PointLight>()
            .component::<exact_game::Animation>()
            .component::<exact_game::Visible>()
            .checked::<Mesh>(|mesh| mesh.validate().map_err(DataError::new));
        types
    }
    pub fn component<C: Component>(&mut self) -> &mut Self {
        self.checked::<C>(|_| Ok(()))
    }
    /// Explicit Rust constructor for opaque fields (for example bulk buffers).
    /// Its authored input is itself Data; no expression evaluator is involved.
    pub fn constructed<A: Data, C: Component>(
        &mut self,
        make: fn(A) -> Result<C, DataError>,
        check: fn(&C) -> Result<(), DataError>,
    ) -> &mut Self {
        self.checked::<C>(check);
        self.types.get_mut(C::NAME).unwrap().defaults =
            || exact_game::json::to_string(&A::default());
        self.types.get_mut(C::NAME).unwrap().bake = Box::new(move |text| {
            let value = make(exact_game::json::from_str_strict::<A>(text)?)?;
            check(&value)?;
            Ok(bin::to_vec(&value))
        });
        self
    }
    /// A typed domain boundary for constraints beyond Data's field/variant types.
    pub fn checked<C: Component>(&mut self, check: fn(&C) -> Result<(), DataError>) -> &mut Self {
        assert!(
            C::NAME != "Parent",
            "scene parent references use the parent field"
        );
        assert!(
            !self.types.contains_key(C::NAME),
            "duplicate scene component {}",
            C::NAME
        );
        self.types.insert(
            C::NAME,
            Registration {
                bake: Box::new(move |text| {
                    let value = exact_game::json::from_str_strict::<C>(text)?;
                    check(&value)?;
                    Ok(bin::to_vec(&value))
                }),
                decode: Box::new(move |bytes, budget| {
                    let value = bin::from_slice_in::<C>(bytes, Some(budget))?;
                    check(&value)?;
                    Ok(Box::new(value))
                }),
                defaults: || exact_game::json::to_string(&C::default()),
                register: |world| {
                    world.register::<C>();
                },
            },
        );
        self
    }
    /// Authoring examples/default metadata emitted by the very same Data writer.
    /// Field types are enforced by the registered Rust reader, including enum arms
    /// absent from the default example. Opaque constructors expose their input default.
    pub fn defaults(&self) -> Result<BTreeMap<&'static str, String>, DataError> {
        self.types
            .iter()
            .map(|(name, ty)| Ok((*name, (ty.defaults)()?)))
            .collect()
    }
    pub fn register(&self, world: &mut World) {
        for ty in self.types.values() {
            (ty.register)(world);
        }
        world
            .register::<Parent>()
            .register::<GeneratedBy>()
            .register_resource::<SceneIdentity>();
    }
    /// Check all types, references and embedded-asset bytes before world mutation.
    pub fn prepare(
        &self,
        text: &str,
        assets: &[exact_game::Asset],
    ) -> Result<PreparedScene, String> {
        let bytes = unhex(text)?;
        let payload = bytes
            .strip_prefix(b"EXSCENE\0\x01")
            .ok_or("unsupported scene artifact; rebake content")?;
        let budget = LoadBudget::new(MAX_SCENE_ALLOCATIONS);
        check_entity_count(payload, &budget).map_err(|e| e.to_string())?;
        let scene: Scene = bin::from_slice_in(payload, Some(&budget)).map_err(|e| e.to_string())?;
        let digest = digest(payload);
        let names: BTreeSet<_> = scene.entities.iter().map(|r| r.id.as_str()).collect();
        if names.len() != scene.entities.len() {
            return Err("duplicate scene identity".into());
        }
        if names.len() > MAX_SCENE_ENTITIES {
            return Err("scene exceeds entity limit".into());
        }
        for (name, expected) in &scene.assets {
            let asset = assets
                .iter()
                .find(|a| a.name == name)
                .ok_or_else(|| format!("missing scene asset {name}; rebuild assets or restart"))?;
            if digest_bytes(asset.bytes) != *expected {
                return Err(format!("scene asset {name} digest differs; rebake against the behavior module's asset bytes"));
            }
        }
        let mut rows = Vec::new();
        for row in &scene.entities {
            if !valid_key(&row.id) {
                return Err(format!("invalid scene identity {}", row.id));
            }
            let mut seen = BTreeSet::new();
            let mut at = row;
            while !at.parent.is_empty() {
                if seen.len() >= 256 || !seen.insert(at.id.as_str()) {
                    return Err(format!("parent cycle at {}", row.id));
                }
                at = scene
                    .entities
                    .iter()
                    .find(|r| r.id == at.parent)
                    .ok_or_else(|| format!("{}: missing parent {}", at.id, at.parent))?;
            }
            let mut components = Vec::new();
            for (name, bytes) in &row.components {
                let ty = self
                    .types
                    .get(name.as_str())
                    .ok_or_else(|| format!("{}: unregistered component {name}", row.id))?;
                let component =
                    (ty.decode)(bytes, &budget).map_err(|e| format!("{}.{}: {e}", row.id, name))?;
                if name == Mesh::NAME {
                    if let Mesh::Asset(name) = bin::from_slice_in::<Mesh>(bytes, Some(&budget))
                        .map_err(|e| e.to_string())?
                    {
                        if !scene.assets.contains_key(&name) {
                            return Err(format!("{}: undeclared asset {name}", row.id));
                        }
                    }
                }
                components.push(component);
            }
            rows.push((row.id.clone(), row.parent.clone(), components));
        }
        Ok(PreparedScene {
            digest,
            assets: scene.assets,
            rows,
        })
    }
}
// Scene uses the normal Data encoding. Check its declared cardinality before
// that reader allocates the first Row; unknown fields remain safely skippable.
fn check_entity_count(payload: &[u8], budget: &LoadBudget) -> Result<(), DataError> {
    let mut reader = bin::Decoder::for_load(payload, Some(budget));
    reader.begin_struct()?;
    while let Some(name) = reader.field()? {
        if name == "entities" {
            reader.begin_seq()?;
            if reader
                .sequence_len()
                .is_some_and(|n| n > MAX_SCENE_ENTITIES)
            {
                return Err(DataError::new("scene exceeds entity limit"));
            }
            return Ok(());
        }
        reader.skip()?;
    }
    reader.finish()
}
/// Fully validated data ready to insert through the same World::insert as Rust setup.
pub struct PreparedScene {
    pub digest: String,
    assets: BTreeMap<String, String>,
    rows: Vec<(String, String, Vec<Prepared>)>,
}
impl PreparedScene {
    pub fn instantiate(self, world: &mut World) -> Result<(), String> {
        for (name, _, _) in &self.rows {
            if world.named(name).is_some() {
                return Err(format!(
                    "scene identity {name} already exists; restart into a fresh world"
                ));
            }
        }
        let entities: BTreeMap<_, _> = self
            .rows
            .iter()
            .map(|(name, _, _)| (name.clone(), world.spawn_named(name, ())))
            .collect();
        for (name, parent, components) in self.rows {
            let entity = entities[&name];
            for component in components {
                component.insert(world, entity);
            }
            if !parent.is_empty() {
                world.insert(entity, Parent(entities[&parent]));
            }
        }
        world.insert_resource(SceneIdentity {
            digest: self.digest,
            assets: self.assets,
        });
        world.propagate();
        Ok(())
    }
}
pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(bytes: &[u8]) -> String {
    digest_bytes(bytes)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Result<Vec<u8>, String> {
    if text.len() > MAX_SCENE_BYTES * 2 || !text.len().is_multiple_of(2) {
        return Err("invalid scene artifact size".into());
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|s| {
            let digit = |b: u8| {
                (b as char)
                    .to_digit(16)
                    .ok_or_else(|| "scene artifact must be hexadecimal".to_string())
            };
            Ok((digit(s[0])? * 16 + digit(s[1])?) as u8)
        })
        .collect()
}
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.split('/').all(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        })
}
