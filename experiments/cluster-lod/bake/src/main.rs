use clod_bake::{Result, bake, loaders, procedural};
use clod_format::{Config, Reader};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    time::Instant,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn rss() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes the supplied struct on success; RUSAGE_SELF is a valid selector.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return 0;
    }
    // SAFETY: successful getrusage initialized usage.
    let bytes = unsafe { usage.assume_init() }.ru_maxrss as u64;
    if cfg!(target_os = "macos") {
        bytes
    } else {
        bytes * 1024
    }
}
fn stats(reader: &Reader<'_>) -> Value {
    let depth = reader.groups.iter().map(|g| g.depth).max().unwrap_or(0);
    let mut triangles = vec![0u64; depth as usize + 1];
    let mut clusters = vec![0u64; depth as usize + 1];
    for c in reader.clusters {
        triangles[c.depth as usize] += c.triangle_count as u64;
        clusters[c.depth as usize] += 1;
    }
    json!({"source_triangles":reader.header.source_triangles,"source_vertices":reader.header.source_vertices,
        "clusters":reader.clusters.len(),"groups":reader.groups.len(),"dag_depth":depth,
        "triangles_per_depth":triangles,"clusters_per_depth":clusters,"pages":reader.pages.len(),
        "file_bytes":reader.header.file_bytes,"bytes_per_source_triangle":reader.header.file_bytes as f64/reader.header.source_triangles as f64,
        "bvh_nodes":reader.nodes.len(),"source_sha256":hex(&reader.header.source_sha256)})
}
fn argument<'a>(args: &'a [String], flag: &str) -> Result<&'a str> {
    let pos = args
        .iter()
        .position(|s| s == flag)
        .ok_or_else(|| format!("missing {flag}"))?;
    Ok(args
        .get(pos + 1)
        .ok_or_else(|| format!("missing value for {flag}"))?)
}
fn run() -> Result<Value> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--generate") {
        if args.len() != 3 {
            return Err("usage: clod-bake --generate <subdivisions 0..10> <mesh.ply>".into());
        }
        let start = Instant::now();
        let mesh = procedural::octasphere(args[1].parse()?)?;
        let mut output = BufWriter::new(File::create(&args[2])?);
        procedural::write_ply(&mesh, &mut output)?;
        output.flush()?;
        return Ok(
            json!({"output":args[2],"vertices":mesh.positions.len(),"triangles":mesh.indices.len()/3,"seconds":start.elapsed().as_secs_f64(),"peak_rss_bytes":rss()}),
        );
    }
    if args
        .first()
        .is_some_and(|s| s == "--inspect" || s == "--cut")
    {
        let path = args.get(1).ok_or("missing clod file")?;
        let bytes = std::fs::read(path)?;
        let reader = Reader::new(&bytes)?;
        let mut result = stats(&reader);
        if args[0] == "--inspect" {
            let h = reader.header;
            let cfg = h.config;
            result["header"] = json!({
                "magic": String::from_utf8_lossy(&h.magic), "header_bytes": h.header_bytes,
                "flags": h.flags, "root_count": h.root_count,
                "clusters_offset": h.clusters_offset, "groups_offset": h.groups_offset,
                "pages_offset": h.pages_offset, "nodes_offset": h.nodes_offset,
                "geometry_offset": h.geometry_offset,
                "config": {"max_triangles": cfg.max_triangles, "page_bytes": cfg.page_bytes,
                    "partition_size": cfg.partition_size, "vertex_encoding": cfg.vertex_encoding,
                    "normal_weight": cfg.normal_weight, "color_weight": cfg.color_weight,
                    "simplify_ratio": cfg.simplify_ratio, "simplify_threshold": cfg.simplify_threshold,
                    "error_merge_previous": cfg.error_merge_previous,
                    "error_merge_additive": cfg.error_merge_additive}
            });
            result["version"] = json!(reader.header.version);
            result["max_triangles"] = json!(reader.header.config.max_triangles);
            result["page_bytes"] = json!(reader.header.config.page_bytes);
            result["vertex_stride"] = json!(std::mem::size_of::<clod_format::Vertex>());
            result["cluster_stride"] = json!(std::mem::size_of::<clod_format::Cluster>());
            result["page_table"]=json!(reader.pages.iter().map(|p| json!({"offset":p.offset,"bytes":p.byte_length,"sha256":hex(&p.sha256),"first_cluster":p.first_cluster,"clusters":p.cluster_count,"vertices":p.vertex_count,"indices":p.index_count})).collect::<Vec<_>>());
        } else {
            let threshold: f32 = argument(&args, "--threshold")?.parse()?;
            if !threshold.is_finite() || threshold < 0.0 || threshold == f32::MAX {
                return Err("threshold must be finite, nonnegative and less than f32::MAX".into());
            }
            let path = argument(&args, "--obj")?;
            let mut out = BufWriter::new(File::create(path)?);
            let mut vertices = 0u64;
            let mut triangles = 0u64;
            let mut selected = 0u64;
            for (i, c) in reader
                .clusters
                .iter()
                .enumerate()
                .filter(|(_, c)| c.selected(threshold))
            {
                let (vs, is) = reader.geometry(i).ok_or("missing geometry")?;
                writeln!(out, "g cluster_{i}_depth_{}", c.depth)?;
                for v in vs {
                    writeln!(
                        out,
                        "v {} {} {}",
                        v.position[0], v.position[1], v.position[2]
                    )?;
                }
                for t in is.chunks_exact(3) {
                    writeln!(
                        out,
                        "f {} {} {}",
                        vertices + t[0] as u64 + 1,
                        vertices + t[1] as u64 + 1,
                        vertices + t[2] as u64 + 1
                    )?;
                }
                vertices += vs.len() as u64;
                triangles += is.len() as u64 / 3;
                selected += 1;
            }
            out.flush()?;
            result["cut"] = json!({"threshold":threshold,"triangles":triangles,"clusters":selected,"vertices":vertices,"obj":path});
        }
        result["output_sha256"] = json!(hex(&Sha256::digest(&bytes)));
        return Ok(result);
    }
    if args.len() < 2 {
        return Err("usage: clod-bake <input> <output.clod> [--max-triangles 128] [--page-mib 32]; --inspect <file>; --cut <file> --threshold <e> --obj <file>; --generate <subdivisions> <file.ply>".into());
    }
    let mut config = Config::default();
    let mut i = 2;
    while i < args.len() {
        let value = args.get(i + 1).ok_or("missing option value")?;
        match args[i].as_str() {
            "--max-triangles" => config.max_triangles = value.parse()?,
            "--page-mib" => {
                let mib: f64 = value.parse()?;
                if !mib.is_finite() || !(0.00390625..=128.0).contains(&mib) {
                    return Err("page-mib must be between 1/256 and 128".into());
                }
                config.page_bytes = (mib * 1024.0 * 1024.0) as u32;
            }
            flag => return Err(format!("unknown option {flag}").into()),
        }
        i += 2;
    }
    let start = Instant::now();
    let (mut mesh, hash) = loaders::load(Path::new(&args[0]))?;
    let load = start.elapsed().as_secs_f64();
    let baked = bake(&mut mesh, config, hash)?;
    let start = Instant::now();
    let mut out = BufWriter::new(File::create(&args[1])?);
    out.write_all(&baked.bytes)?;
    out.flush()?;
    let write = start.elapsed().as_secs_f64();
    let reader = Reader::new(&baked.bytes)?;
    let mut result = stats(&reader);
    result["input"] = json!(args[0]);
    result["output"] = json!(args[1]);
    result["seconds"] = json!({"load":load,"normals":baked.normals_seconds,"build":baked.build_seconds,"encode":baked.encode_seconds,"write":write});
    result["peak_rss_bytes"] = json!(rss());
    result["output_sha256"] = json!(hex(&Sha256::digest(&baked.bytes)));
    Ok(result)
}
fn main() {
    match run() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            println!("{}", json!({"error":error.to_string()}));
            std::process::exit(1);
        }
    }
}
