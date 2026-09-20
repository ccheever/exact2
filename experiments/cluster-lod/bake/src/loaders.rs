mod gltf;
mod ply;
use crate::{Mesh, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

struct Hashed<R> {
    inner: R,
    hash: Sha256,
}
impl<R: Read> Read for Hashed<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(out)?;
        self.hash.update(&out[..n]);
        Ok(n)
    }
}
/// Hashes every primary source byte, including trailing data, while streaming native loaders.
pub fn load(path: &Path) -> Result<(Mesh, [u8; 32])> {
    let mut input = BufReader::with_capacity(
        1024 * 1024,
        Hashed {
            inner: File::open(path)?,
            hash: Sha256::new(),
        },
    );
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mesh = match ext.as_str() {
        "ply" => ply::read(&mut input)?,
        "obj" => read_obj(&mut input)?,
        "stl" => read_stl(&mut input)?,
        "gltf" | "glb" => {
            let mut bytes = Vec::new();
            input.read_to_end(&mut bytes)?;
            gltf::read(&bytes, path)?
        }
        _ => return Err(format!("unsupported mesh extension: {ext}").into()),
    };
    std::io::copy(&mut input, &mut std::io::sink())?;
    let hash = input.into_inner().hash.finalize().into();
    mesh.validate()?;
    Ok((mesh, hash))
}
pub fn read_obj(mut input: impl BufRead) -> Result<Mesh> {
    let mut mesh = Mesh::default();
    let mut line = String::new();
    let mut face = Vec::new();
    while input.read_line(&mut line)? > 0 {
        let mut parts = line.split('#').next().unwrap_or("").split_whitespace();
        match parts.next() {
            Some("v") => {
                let mut p = [0.0; 3];
                for v in &mut p {
                    *v = parts.next().ok_or("missing OBJ coordinate")?.parse()?;
                }
                mesh.positions.push(p);
            }
            Some("f") => {
                face.clear();
                for part in parts {
                    let raw: i64 = part.split('/').next().ok_or("missing OBJ index")?.parse()?;
                    let id = if raw > 0 {
                        raw - 1
                    } else {
                        mesh.positions.len() as i64 + raw
                    };
                    if raw == 0 || id < 0 || id >= mesh.positions.len() as i64 {
                        return Err("OBJ index out of range".into());
                    }
                    face.push(id as u32);
                }
                if face.len() < 3 {
                    return Err("OBJ face has fewer than three vertices".into());
                }
                for i in 1..face.len() - 1 {
                    mesh.indices
                        .extend_from_slice(&[face[0], face[i], face[i + 1]]);
                }
            }
            _ => {}
        }
        line.clear();
    }
    mesh.validate()?;
    Ok(mesh)
}
pub fn read_stl(mut input: impl Read) -> Result<Mesh> {
    let mut header = [0u8; 84];
    input.read_exact(&mut header)?;
    let count = u32::from_le_bytes(header[80..84].try_into()?) as usize;
    if count > u32::MAX as usize / 3 {
        return Err("STL too large".into());
    }
    let mut mesh = Mesh::default();
    let mut weld = HashMap::new();
    for _ in 0..count {
        let mut record = [0u8; 50];
        input.read_exact(&mut record)?;
        for k in 0..3 {
            let p: [f32; 3] = std::array::from_fn(|j| {
                let start = 12 + k * 12 + j * 4;
                let v =
                    f32::from_le_bytes(record[start..start + 4].try_into().expect("four bytes"));
                if v == 0.0 { 0.0 } else { v }
            });
            if p.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite STL position".into());
            }
            let id = *weld.entry(p.map(f32::to_bits)).or_insert_with(|| {
                let id = mesh.positions.len() as u32;
                mesh.positions.push(p);
                id
            });
            mesh.indices.push(id);
        }
    }
    mesh.validate()?;
    Ok(mesh)
}
