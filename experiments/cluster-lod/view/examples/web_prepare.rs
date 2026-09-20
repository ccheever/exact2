//! Cache-only camera metadata and optional conventional buffers for the browser host.
#[path = "../src/bin/prepare.rs"]
mod prepare;
use clod_format::Reader;
use clod_view::scene::Scene;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let input = args.get(1).ok_or("asset path")?;
    let out = PathBuf::from(args.get(2).ok_or("cache directory")?);
    std::fs::create_dir_all(&out)?;
    let bytes = std::fs::read(input)?;
    let r = Reader::new(&bytes)?;
    let digest = format!(
        "{:x}",
        Sha256::digest(&bytes[..r.header.geometry_offset as usize])
    );
    if args.iter().any(|a| a == "--naive") {
        let baselines = prepare::baseline(&r);
        let mut file = std::fs::File::create(out.join("naive.bin"))?;
        for b in &baselines {
            file.write_all(bytemuck::cast_slice(&[
                b.vertices.len() as u32,
                b.indices.len() as u32,
            ]))?;
            file.write_all(bytemuck::cast_slice(&b.vertices))?;
            file.write_all(bytemuck::cast_slice(&b.indices))?;
        }
        println!(
            "{}",
            json!({"baseline_chunks":baselines.len(),"bytes":file.metadata()?.len()})
        );
    } else {
        let mut scenes = std::collections::BTreeMap::new();
        for layout in ["single", "ring:12", "avenue:25", "grid:400"] {
            scenes.insert(layout, Scene::layout(&r, layout)?);
        }
        let data = json!({"metadata_sha256":digest,"scenes":scenes});
        std::fs::write(out.join("scenes.json"), serde_json::to_vec(&data)?)?;
        println!(
            "{}",
            json!({"scenes":scenes.len(),"metadata_sha256":digest})
        );
    }
    Ok(())
}
