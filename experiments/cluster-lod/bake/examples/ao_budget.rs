use std::{path::Path, time::Instant};
fn main() -> clod_bake::Result<()> {
    let file = std::env::args().nth(1).ok_or("source path required")?;
    let (mut mesh, _) = clod_bake::loaders::load(Path::new(&file))?;
    mesh.compute_normals();
    let start = Instant::now();
    let (values, proxy) = clod_bake::ao::bake(&mesh)?;
    println!(
        "vertices={} proxy_triangles={} rays={} ao_seconds={} min={} mean={}",
        values.len(),
        proxy,
        values.len() * 16,
        start.elapsed().as_secs_f64(),
        values.iter().min().unwrap(),
        values.iter().map(|&v| v as u64).sum::<u64>() as f64 / values.len() as f64
    );
    Ok(())
}
