//! Native content bake. No source file is a Rust include or a build.rs input.
use crate::{
    digest_bytes, hex,
    source::{parse, Location, Node},
    Asset, Row, Scene, Types,
};
use exact_game::{bin, Mesh};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub struct Baked {
    /// Runtime data only; safe to carry in a Contract string construction argument.
    pub content: String,
    pub digest: String,
    /// Development sidecar, never inserted into the runtime artifact.
    pub source_map: Value,
}
struct Authored {
    id: String,
    parent: String,
    components: BTreeMap<String, Node>,
    at: Location,
    parent_at: Location,
    calls: Vec<Location>,
}
struct Compiler<'a> {
    types: &'a Types,
    assets: &'a [Asset<'a>],
    files: BTreeMap<String, String>,
    dependencies: BTreeMap<String, String>,
    rows: Vec<Authored>,
    errors: Vec<String>,
    reads: usize,
    work: usize,
}
/// Read sources once, expand explicit parameters and validate via actual Data readers.
pub fn compile(
    path: impl AsRef<Path>,
    types: &Types,
    assets: &[Asset<'_>],
) -> Result<Baked, String> {
    let mut c = Compiler {
        types,
        assets,
        files: BTreeMap::new(),
        dependencies: BTreeMap::new(),
        rows: vec![],
        errors: vec![],
        reads: 64 * 1024 * 1024,
        work: 256 * 1024 * 1024,
    };
    let path =
        fs::canonicalize(path.as_ref()).map_err(|e| format!("{}: {e}", path.as_ref().display()))?;
    c.expand(&path, "", &BTreeMap::new(), &[], &mut vec![])?;
    let mut names = BTreeMap::new();
    for row in &c.rows {
        if let Some(old) = names.insert(row.id.clone(), row) {
            c.errors.push(row.at.error(format!(
                "duplicate identity {}; first at {}",
                row.id,
                old.at.error("declaration")
            )));
        }
    }
    for row in &c.rows {
        let mut at = row;
        let mut seen = BTreeSet::new();
        while !at.parent.is_empty() {
            if seen.len() >= 256 || !seen.insert(at.id.as_str()) {
                c.errors
                    .push(row.parent_at.error(format!("parent cycle at {}", row.id)));
                break;
            }
            match names.get(&at.parent) {
                Some(parent) => at = parent,
                None => {
                    c.errors
                        .push(at.parent_at.error(format!("missing parent {}", at.parent)));
                    break;
                }
            }
        }
    }
    let mut scene = Scene {
        assets: c.dependencies.clone(),
        ..Scene::default()
    };
    let mut mappings = serde_json::Map::new();
    for row in &c.rows {
        let mut components = BTreeMap::new();
        for (name, node) in &row.components {
            let Some(ty) = c.types.types.get(name.as_str()) else {
                c.errors.push(node.at.error(format!(
                    "unregistered scene component {name}; register its Rust Component type"
                )));
                continue;
            };
            match (ty.bake)(&node.value.to_string()) {
                Ok(bytes) => {
                    if name == "Mesh" {
                        if let Ok(Mesh::Asset(asset)) = bin::from_slice::<Mesh>(&bytes) {
                            if !scene.assets.contains_key(&asset) {
                                c.errors.push(node.at.error(format!(
                                    "undeclared asset {asset}; use an explicit $asset reference"
                                )));
                            }
                        }
                    }
                    components.insert(name.clone(), bytes);
                }
                Err(e) => c
                    .errors
                    .push(node.error_at(&e.path, &format!("{name}.{}: {}", e.path, e.message))),
            }
        }
        scene.entities.push(Row {
            id: row.id.clone(),
            parent: row.parent.clone(),
            components,
        });
        let fields: BTreeMap<_, _> = row
            .components
            .iter()
            .map(|(k, v)| (k, v.at.json()))
            .collect();
        mappings.insert(row.id.clone(), json!({"declaration":row.at.json(),"instances":row.calls.iter().map(Location::json).collect::<Vec<_>>(),"components":fields}));
    }
    if !c.errors.is_empty() {
        return Err(c.errors.join("\n"));
    }
    // Refuse a source/asset race instead of publishing content from moving inputs.
    for (file, expected) in &c.files {
        if digest_bytes(&bounded_read(Path::new(file), &mut c.reads)?) != *expected {
            return Err(format!("{file}: changed during scene bake; rebake"));
        }
    }
    let payload = bin::to_vec(&scene);
    let digest = digest_bytes(&payload);
    let mut bytes = b"EXSCENE\0\x01".to_vec();
    bytes.extend(payload);
    let content = hex(&bytes);
    types.prepare(&content, assets)?;
    Ok(Baked {
        content,
        digest: digest.clone(),
        source_map: json!({"version":1,"sceneDigest":digest,"files":c.files,"entities":mappings}),
    })
}
impl Compiler<'_> {
    fn read(&mut self, path: &Path) -> Result<Node, String> {
        let bytes = bounded_read(path, &mut self.reads)?;
        let file = path.to_string_lossy().into_owned();
        let hash = digest_bytes(&bytes);
        if self.files.get(&file).is_some_and(|old| *old != hash) {
            return Err(format!("{file}: changed during scene bake"));
        }
        self.files.insert(file.clone(), hash);
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("{file}: {e}"))?;
        parse(&file, text)
    }
    fn expand(
        &mut self,
        path: &Path,
        prefix: &str,
        parameters: &BTreeMap<String, Node>,
        calls: &[Location],
        stack: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        if stack.contains(&path.to_path_buf()) || stack.len() >= 32 {
            return Err(format!(
                "{}: fragment dependency cycle or depth exceeds 32",
                path.display()
            ));
        }
        let doc = self.read(path)?;
        doc.keys(&["units", "parameters", "fragments", "assets", "entities"])?;
        if doc.field("units")?.text()? != "metres-y-up" {
            return Err(doc
                .field("units")?
                .at
                .error("expected metres-y-up (right-handed, forward -Z)"));
        }
        let required: BTreeSet<_> = match doc.children.get("parameters") {
            Some(n) => n
                .array()?
                .into_iter()
                .map(|v| Ok(v.text()?.to_owned()))
                .collect::<Result<_, String>>()?,
            None => BTreeSet::new(),
        };
        if let Some(n) = doc.children.get("parameters") {
            if n.array()?.len() != required.len() {
                return Err(n.at.error("duplicate parameter declaration"));
            }
        }
        let supplied: BTreeSet<_> = parameters.keys().cloned().collect();
        if required != supplied {
            return Err(doc.at.error(format!(
                "fragment parameters differ: required {required:?}, supplied {supplied:?}"
            )));
        }
        let mut asset_names = BTreeMap::new();
        if let Some(assets) = doc.children.get("assets") {
            for (alias, node) in assets.object()? {
                node.keys(&["name", "path"])?;
                let name = node.field("name")?.text()?;
                let path = resolve(path, node.field("path")?)?;
                let bytes = bounded_read(&path, &mut self.reads).map_err(|e| node.at.error(e))?;
                let digest = digest_bytes(&bytes);
                let registered = self.assets.iter().find(|a| a.name == name).ok_or_else(|| {
                    node.at.error(format!(
                        "asset {name} is not declared by the baked asset manifest"
                    ))
                })?;
                if registered.digest != digest {
                    return Err(node.at.error(format!(
                        "asset {name} differs from the behavior module; rebuild code/assets first"
                    )));
                }
                self.files
                    .insert(path.to_string_lossy().into_owned(), digest.clone());
                if self
                    .dependencies
                    .insert(name.into(), digest.clone())
                    .is_some_and(|old| old != digest)
                {
                    return Err(node.at.error(format!("conflicting asset {name}")));
                }
                asset_names.insert(alias.clone(), name.to_owned());
            }
        }
        stack.push(path.to_path_buf());
        for original in doc.field("entities")?.array()? {
            let result = (|| {
                let node = substitute(original, parameters, &asset_names, &mut self.work)?;
                node.keys(&[
                    "id",
                    "parent",
                    "components",
                    "fragment",
                    "parameters",
                    "overrides",
                ])?;
                let local = node.field("id")?.text()?;
                if (!local.is_empty() && (!crate::valid_key(local) || local.contains('/')))
                    || (prefix.is_empty() && local.is_empty())
                {
                    return Err(node
                        .field("id")?
                        .at
                        .error("id must be a nonempty local key (empty is the fragment root)"));
                }
                let id = qualify(prefix, local);
                let parent_node = node.children.get("parent");
                let parent = match parent_node {
                    Some(p) => reference(prefix, p.text()?),
                    None => String::new(),
                };
                if let Some(node) = parent_node.filter(|_| parent.is_empty()) {
                    return Err(node
                        .at
                        .error("parent reference must name an entity; $root requires a fragment"));
                }
                if let Some(fragment) = node.children.get("fragment") {
                    if node.children.contains_key("components") {
                        return Err(node
                            .at
                            .error("fragment instance uses explicit overrides, not components"));
                    }
                    let fragments = doc.field("fragments")?.object()?;
                    let target = fragments
                        .get(fragment.text()?)
                        .ok_or_else(|| fragment.at.error("unknown fragment"))?;
                    let file = resolve(path, target)?;
                    let empty = BTreeMap::new();
                    let params = match node.children.get("parameters") {
                        Some(n) => n.object()?,
                        None => &empty,
                    };
                    let begin = self.rows.len();
                    let mut chain = calls.to_vec();
                    chain.push(node.at.clone());
                    self.expand(&file, &id, params, &chain, stack)
                        .map_err(|e| format!("{e}\n{}", node.at.error("fragment instance")))?;
                    if !parent.is_empty() {
                        let root = self.rows[begin..]
                            .iter_mut()
                            .find(|r| r.id == id)
                            .ok_or_else(|| {
                                node.at
                                    .error("parent requires a fragment root with id empty")
                            })?;
                        root.parent = parent;
                        root.parent_at = parent_node.unwrap().at.clone();
                    }
                    if let Some(overrides) = node.children.get("overrides") {
                        for (key, replacement) in overrides.object()? {
                            let target = qualify(&id, key);
                            let row = self.rows[begin..]
                                .iter_mut()
                                .find(|r| r.id == target)
                                .ok_or_else(|| {
                                    replacement
                                        .at
                                        .error(format!("override target {target} does not exist"))
                                })?;
                            for (name, value) in replacement.object()? {
                                row.components.insert(name.clone(), value.clone());
                            }
                        }
                    }
                } else {
                    if node.children.contains_key("parameters")
                        || node.children.contains_key("overrides")
                    {
                        return Err(node
                            .at
                            .error("parameters/overrides require a fragment instance"));
                    }
                    if self.rows.len() >= 100_000 {
                        return Err(node.at.error("scene exceeds entity limit"));
                    }
                    self.rows.push(Authored {
                        id,
                        parent,
                        components: node.field("components")?.object()?.clone(),
                        at: node.field("id")?.at.clone(),
                        parent_at: parent_node.unwrap_or(&node).at.clone(),
                        calls: calls.to_vec(),
                    });
                }
                Ok(())
            })();
            if let Err(e) = result {
                if e.contains("scene work limit") {
                    return Err(e);
                }
                self.errors.push(e);
            }
        }
        stack.pop();
        Ok(())
    }
}
fn resolve(base: &Path, node: &Node) -> Result<PathBuf, String> {
    fs::canonicalize(base.parent().unwrap().join(node.text()?)).map_err(|e| node.at.error(e))
}
fn qualify(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.into()
    } else if local.is_empty() {
        prefix.into()
    } else {
        format!("{prefix}/{local}")
    }
}
fn reference(prefix: &str, key: &str) -> String {
    if let Some(global) = key.strip_prefix('/') {
        global.into()
    } else if key == "$root" {
        prefix.into()
    } else {
        qualify(prefix, key)
    }
}
fn substitute(
    node: &Node,
    parameters: &BTreeMap<String, Node>,
    assets: &BTreeMap<String, String>,
    work: &mut usize,
) -> Result<Node, String> {
    charge(work, node.cost).map_err(|e| node.at.error(e))?;
    if node.value.is_object() {
        if let Some(param) = node.children.get("$param") {
            node.keys(&["$param"])?;
            let value = parameters
                .get(param.text()?)
                .ok_or_else(|| param.at.error("undeclared parameter"))?;
            charge(work, value.cost).map_err(|e| param.at.error(e))?;
            return Ok(value.clone());
        }
        if let Some(asset) = node.children.get("$asset") {
            node.keys(&["$asset"])?;
            let name = assets
                .get(asset.text()?)
                .ok_or_else(|| asset.at.error("undeclared asset reference"))?;
            return Ok(Node {
                cost: name.len(),
                value: Value::String(name.clone()),
                at: node.at.clone(),
                children: BTreeMap::new(),
            });
        }
    }
    let mut result = node.clone();
    for (key, child) in &node.children {
        let next = substitute(child, parameters, assets, work)?;
        if result.value.is_object() {
            result.value[key] = next.value.clone();
        } else if result.value.is_array() {
            result.value[key.parse::<usize>().unwrap()] = next.value.clone();
        }
        result.children.insert(key.clone(), next);
    }
    Ok(result)
}
// Shared across every fragment expansion, asset read, and final race check.
// Charge before allocation; a repeated empty fragment cannot evade the row cap.
fn charge(budget: &mut usize, amount: usize) -> Result<(), String> {
    *budget = budget
        .checked_sub(amount)
        .ok_or("scene work limit exceeded")?;
    Ok(())
}
fn bounded_read(path: &Path, budget: &mut usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let limit = (*budget).min(crate::MAX_SCENE_BYTES);
    if size > limit as u64 {
        return Err(format!("{}: scene work limit exceeded", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err(format!("{}: scene work limit exceeded", path.display()));
    }
    charge(budget, bytes.len())?;
    Ok(bytes)
}
/// Per-game CLI shared by the two tiny typed entrypoints. Writes only after validation.
pub fn main(types: Types, assets: &[Asset<'_>]) {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = (|| {
        if args.len() != 2 {
            return Err("usage: scene-bake <scene.json> <output-directory>".into());
        }
        let baked = compile(&args[0], &types, assets)?;
        let out = Path::new(&args[1]);
        fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let contract = format!("// Generated scene data; Rust Component/Data declarations are the type authority.\nfn sceneContent(): string = {}\n", serde_json::to_string(&baked.content).unwrap());
        write_changed(&out.join("scene.contract"), contract.as_bytes())?;
        write_changed(&out.join("scene.binhex"), baked.content.as_bytes())?;
        if std::env::var("EXACT_UPDATE_TRUST").as_deref() == Ok("development") {
            write_changed(
                &out.join("scene.map.json"),
                serde_json::to_string_pretty(&baked.source_map)
                    .unwrap()
                    .as_bytes(),
            )?;
        } else if out.join("scene.map.json").exists() {
            fs::remove_file(out.join("scene.map.json")).map_err(|e| e.to_string())?;
        }
        println!("scene content rebake {} — Restart instantiates content, Continue retains saved entities", baked.digest);
        Ok::<_, String>(())
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn write_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)
        .and_then(|()| fs::rename(tmp, path))
        .map_err(|e| format!("{}: {e}", path.display()))
}
