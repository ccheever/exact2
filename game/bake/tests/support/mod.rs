//! Real bake payloads for headless simulation tests; no substitute models.
use std::{collections::BTreeMap, path::Path};
pub fn assets(path: impl AsRef<Path>) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let path = path.as_ref();
    let (model, textures) = exact_game_bake::assets(path)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid art stem")?;
    let name = format!("{stem}.model");
    let mut result = BTreeMap::new();
    result.insert(name.clone(), exact_game_bake::encode(&name, &model)?);
    for (name, texture) in textures {
        result.insert(name.clone(), exact_game_bake::encode(&name, &texture)?);
    }
    Ok(result)
}
