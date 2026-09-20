use crate::{Mesh, Result};
use std::io::BufRead;

#[derive(Clone, Copy)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}
impl Scalar {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return Err(format!("unsupported PLY scalar {s}").into()),
        })
    }
    fn binary(self, input: &mut impl BufRead, big: bool) -> Result<f64> {
        let mut b = [0u8; 8];
        let len = match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        };
        input.read_exact(&mut b[..len])?;
        if big {
            b[..len].reverse();
        }
        Ok(match self {
            Self::I8 => b[0] as i8 as f64,
            Self::U8 => b[0] as f64,
            Self::I16 => i16::from_le_bytes(b[..2].try_into()?) as f64,
            Self::U16 => u16::from_le_bytes(b[..2].try_into()?) as f64,
            Self::I32 => i32::from_le_bytes(b[..4].try_into()?) as f64,
            Self::U32 => u32::from_le_bytes(b[..4].try_into()?) as f64,
            Self::F32 => f32::from_le_bytes(b[..4].try_into()?) as f64,
            Self::F64 => f64::from_le_bytes(b),
        })
    }
}
struct Property {
    name: String,
    scalar: Scalar,
    list: Option<Scalar>,
}
struct Element {
    name: String,
    count: usize,
    properties: Vec<Property>,
}
fn uint(n: f64, max: usize) -> Result<usize> {
    if !n.is_finite() || n < 0.0 || n.fract() != 0.0 || n > max as f64 {
        Err("invalid PLY integer".into())
    } else {
        Ok(n as usize)
    }
}
pub fn read(input: &mut impl BufRead) -> Result<Mesh> {
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != "ply" {
        return Err("missing PLY magic".into());
    }
    let mut encoding = String::new();
    let mut elements: Vec<Element> = Vec::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return Err("truncated PLY header".into());
        }
        let parts: Vec<_> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["format", f, "1.0"] => encoding = (*f).into(),
            ["element", name, count] => elements.push(Element {
                name: (*name).into(),
                count: count.parse()?,
                properties: Vec::new(),
            }),
            ["property", ty, name] => elements
                .last_mut()
                .ok_or("property without element")?
                .properties
                .push(Property {
                    name: (*name).into(),
                    scalar: Scalar::parse(ty)?,
                    list: None,
                }),
            ["property", "list", count, ty, name] => elements
                .last_mut()
                .ok_or("property without element")?
                .properties
                .push(Property {
                    name: (*name).into(),
                    scalar: Scalar::parse(ty)?,
                    list: Some(Scalar::parse(count)?),
                }),
            ["end_header"] => break,
            ["comment", ..] | ["obj_info", ..] | [] => {}
            _ => return Err("unsupported PLY header line".into()),
        }
    }
    if !["ascii", "binary_little_endian", "binary_big_endian"].contains(&encoding.as_str()) {
        return Err("unsupported PLY encoding".into());
    }
    let ascii = encoding == "ascii";
    let big = encoding == "binary_big_endian";
    let mut mesh = Mesh::default();
    for e in elements {
        if e.count > u32::MAX as usize {
            return Err("PLY element too large".into());
        }
        let has = |name: &str| {
            e.properties
                .iter()
                .any(|p| p.name == name && p.list.is_none())
        };
        if e.name == "vertex" && !(has("x") && has("y") && has("z")) {
            return Err("PLY lacks xyz".into());
        }
        let normals = has("nx") && has("ny") && has("nz");
        let colors = has("red") && has("green") && has("blue");
        if e.name == "vertex" && colors {
            mesh.colors = Some(Vec::new());
        }
        for _ in 0..e.count {
            line.clear();
            if ascii && input.read_line(&mut line)? == 0 {
                return Err("truncated ASCII PLY row".into());
            }
            let mut words = line.split_whitespace();
            let mut number = |ty: Scalar| -> Result<f64> {
                if ascii {
                    Ok(words.next().ok_or("missing ASCII PLY field")?.parse()?)
                } else {
                    ty.binary(input, big)
                }
            };
            let mut p = [0.0; 3];
            let mut n = [0.0; 3];
            let mut c = [255u8; 4];
            let mut face = None;
            for prop in &e.properties {
                if let Some(ct) = prop.list {
                    let count = uint(number(ct)?, 1_000_000)?;
                    let wanted = e.name == "face"
                        && ["vertex_indices", "vertex_index"].contains(&prop.name.as_str());
                    if wanted && count != 3 {
                        return Err("PLY requires triangular faces".into());
                    }
                    if wanted {
                        let mut tri = [0u32; 3];
                        for index in &mut tri {
                            *index = uint(number(prop.scalar)?, u32::MAX as usize)? as u32;
                        }
                        face = Some(tri);
                    } else {
                        for _ in 0..count {
                            number(prop.scalar)?;
                        }
                    }
                } else {
                    let value = number(prop.scalar)?;
                    if e.name == "vertex" {
                        match prop.name.as_str() {
                            "x" => p[0] = value as f32,
                            "y" => p[1] = value as f32,
                            "z" => p[2] = value as f32,
                            "nx" => n[0] = value as f32,
                            "ny" => n[1] = value as f32,
                            "nz" => n[2] = value as f32,
                            "red" | "green" | "blue" | "alpha" => {
                                if !matches!(prop.scalar, Scalar::U8) {
                                    return Err("PLY colors must be uchar".into());
                                }
                                let k = match prop.name.as_str() {
                                    "red" => 0,
                                    "green" => 1,
                                    "blue" => 2,
                                    _ => 3,
                                };
                                c[k] = uint(value, 255)? as u8;
                            }
                            _ => {}
                        }
                    }
                }
            }
            match e.name.as_str() {
                "vertex" => {
                    mesh.positions.push(p);
                    if normals {
                        mesh.normals.push(n);
                    }
                    if let Some(cs) = &mut mesh.colors {
                        cs.push(c);
                    }
                }
                "face" => mesh
                    .indices
                    .extend_from_slice(&face.ok_or("PLY face lacks vertex_indices")?),
                _ => {}
            }
        }
    }
    mesh.validate()?;
    Ok(mesh)
}
