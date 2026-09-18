//! Narrow binary glTF ingestion for the engine's first real asset.
//!
//! The accepted subset is deliberately explicit: one triangle primitive, one skin,
//! embedded PNG base colour, and LINEAR translation/rotation/scale channels. It is
//! enough for Khronos Fox while malformed or wider files fail by field name.
use glam::{Mat4, Quat, Vec3, Vec4, Vec4Swizzles};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Cursor;

/// Skinned position, normal and texture coordinates for one vertex.
type SampledVertex = ([f32; 3], [f32; 3], [f32; 2]);

#[derive(Clone)]
pub(crate) struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy)]
struct SourceVertex {
    position: Vec3,
    normal: Vec3,
    uv: [f32; 2],
    joints: [u16; 4],
    weights: [f32; 4],
}

#[derive(Clone, Copy)]
struct Node {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    parent: Option<usize>,
}

#[derive(Clone, Copy)]
enum Path {
    Translation,
    Rotation,
    Scale,
}

struct Channel {
    node: usize,
    path: Path,
    times: Vec<f32>,
    values: Vec<Vec4>,
}

struct Clip {
    duration: f32,
    channels: Vec<Channel>,
}

/// One decoded primitive and its animation data. CPU skinning keeps the first
/// implementation independent of bind-group and joint-count limits.
pub(crate) struct Model {
    source: Vec<SourceVertex>,
    pub indices: Vec<u32>,
    nodes: Vec<Node>,
    joints: Vec<usize>,
    inverse_bind: Vec<Mat4>,
    clips: BTreeMap<String, Clip>,
    pub image: Image,
}

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let (doc, bin) = glb(bytes)?;
        let meshes = array(&doc, "meshes")?;
        let primitive = meshes
            .first()
            .and_then(|m| m["primitives"].as_array())
            .filter(|p| p.len() == 1)
            .and_then(|p| p.first())
            .ok_or("glTF meshes: expected one primitive")?;
        if meshes.len() != 1 || primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
            return Err("glTF meshes: expected one triangle mesh".into());
        }
        let attrs = primitive["attributes"]
            .as_object()
            .ok_or("glTF primitive.attributes is missing")?;
        let positions = read_f32(&doc, bin, index(attrs.get("POSITION"), "POSITION")?, 3)?;
        let uvs = read_f32(&doc, bin, index(attrs.get("TEXCOORD_0"), "TEXCOORD_0")?, 2)?;
        let joints = read_u16(&doc, bin, index(attrs.get("JOINTS_0"), "JOINTS_0")?, 4)?;
        let weights = read_f32(&doc, bin, index(attrs.get("WEIGHTS_0"), "WEIGHTS_0")?, 4)?;
        let count = positions.len() / 3;
        if count == 0
            || uvs.len() != count * 2
            || joints.len() != count * 4
            || weights.len() != count * 4
        {
            return Err("glTF vertex attributes have different counts".into());
        }
        let indices = if let Some(i) = primitive.get("indices") {
            read_indices(&doc, bin, index(Some(i), "indices")?)?
        } else {
            (0..count as u32).collect()
        };
        if indices.len() % 3 != 0 || indices.iter().any(|&i| i as usize >= count) {
            return Err("glTF primitive has invalid triangle indices".into());
        }
        let mut normals = vec![Vec3::ZERO; count];
        for triangle in indices.chunks_exact(3) {
            let a = Vec3::from_slice(&positions[triangle[0] as usize * 3..]);
            let b = Vec3::from_slice(&positions[triangle[1] as usize * 3..]);
            let c = Vec3::from_slice(&positions[triangle[2] as usize * 3..]);
            let n = (b - a).cross(c - a);
            for &i in triangle {
                normals[i as usize] += n;
            }
        }
        for normal in &mut normals {
            *normal = normal.try_normalize().unwrap_or(Vec3::Y);
        }
        let source = (0..count)
            .map(|i| SourceVertex {
                position: Vec3::from_slice(&positions[i * 3..]),
                normal: normals[i],
                uv: [uvs[i * 2], uvs[i * 2 + 1]],
                joints: joints[i * 4..i * 4 + 4].try_into().unwrap(),
                weights: weights[i * 4..i * 4 + 4].try_into().unwrap(),
            })
            .collect();
        let nodes_json = array(&doc, "nodes")?;
        let mut nodes: Vec<Node> = nodes_json
            .iter()
            .map(|n| Node {
                translation: vec3(n.get("translation"), Vec3::ZERO),
                rotation: quat(n.get("rotation"), Quat::IDENTITY),
                scale: vec3(n.get("scale"), Vec3::ONE),
                parent: None,
            })
            .collect();
        for (parent, n) in nodes_json.iter().enumerate() {
            if let Some(children) = n.get("children").and_then(Value::as_array) {
                for child in children {
                    let child = child.as_u64().ok_or("glTF node child is not an index")? as usize;
                    let node = nodes
                        .get_mut(child)
                        .ok_or("glTF node child is out of range")?;
                    if node.parent.replace(parent).is_some() {
                        return Err("glTF node has two parents".into());
                    }
                }
            }
        }
        let skin = array(&doc, "skins")?
            .first()
            .ok_or("glTF skin is missing")?;
        let joints: Vec<usize> = skin["joints"]
            .as_array()
            .ok_or("glTF skin.joints is missing")?
            .iter()
            .map(|v| {
                v.as_u64()
                    .map(|v| v as usize)
                    .ok_or("glTF joint is not an index")
            })
            .collect::<Result<_, _>>()?;
        if joints.iter().any(|&i| i >= nodes.len()) {
            return Err("glTF joint is out of range".into());
        }
        let matrices = read_f32(
            &doc,
            bin,
            index(skin.get("inverseBindMatrices"), "inverseBindMatrices")?,
            16,
        )?;
        if matrices.len() != joints.len() * 16 {
            return Err("glTF inverse bind matrix count differs from joints".into());
        }
        let inverse_bind = matrices
            .chunks_exact(16)
            .map(Mat4::from_cols_slice)
            .collect();
        let mut clips = BTreeMap::new();
        for animation in array(&doc, "animations")? {
            let name = animation["name"]
                .as_str()
                .ok_or("glTF animation name is missing")?
                .to_owned();
            let samplers = animation["samplers"]
                .as_array()
                .ok_or("glTF animation.samplers is missing")?;
            let mut duration = 0.0f32;
            let mut channels = Vec::new();
            for channel in animation["channels"]
                .as_array()
                .ok_or("glTF animation.channels is missing")?
            {
                let sampler = samplers
                    .get(index(channel.get("sampler"), "animation sampler")?)
                    .ok_or("glTF animation sampler is out of range")?;
                if sampler
                    .get("interpolation")
                    .and_then(Value::as_str)
                    .unwrap_or("LINEAR")
                    != "LINEAR"
                {
                    return Err("glTF animation interpolation must be LINEAR".into());
                }
                let times = read_f32(
                    &doc,
                    bin,
                    index(sampler.get("input"), "animation input")?,
                    1,
                )?;
                let path = match channel["target"]["path"].as_str() {
                    Some("translation") => Path::Translation,
                    Some("rotation") => Path::Rotation,
                    Some("scale") => Path::Scale,
                    _ => return Err("glTF animation path is unsupported".into()),
                };
                let lanes = if matches!(path, Path::Rotation) { 4 } else { 3 };
                let raw = read_f32(
                    &doc,
                    bin,
                    index(sampler.get("output"), "animation output")?,
                    lanes,
                )?;
                if raw.len() != times.len() * lanes || times.windows(2).any(|p| p[0] > p[1]) {
                    return Err("glTF animation samples are malformed".into());
                }
                let values = raw
                    .chunks_exact(lanes)
                    .map(|v| Vec4::new(v[0], v[1], v[2], v.get(3).copied().unwrap_or(0.0)))
                    .collect();
                duration = duration.max(times.last().copied().unwrap_or(0.0));
                channels.push(Channel {
                    node: index(channel["target"].get("node"), "animation node")?,
                    path,
                    times,
                    values,
                });
            }
            if duration <= 0.0 || channels.iter().any(|c| c.node >= nodes.len()) {
                return Err(format!(
                    "glTF animation {name} has no duration or an invalid node"
                ));
            }
            clips.insert(name, Clip { duration, channels });
        }
        let image_view = array(&doc, "images")?
            .first()
            .and_then(|v| v.get("bufferView"))
            .map(|v| index(Some(v), "image bufferView"))
            .transpose()?
            .ok_or("glTF embedded image is missing")?;
        let image = decode_png(view_bytes(&doc, bin, image_view)?)?;
        Ok(Self {
            source,
            indices,
            nodes,
            joints,
            inverse_bind,
            clips,
            image,
        })
    }

    pub fn clip_names(&self) -> impl Iterator<Item = &str> {
        self.clips.keys().map(String::as_str)
    }

    /// Write skinned position, normal and UV triples at one deterministic cursor.
    pub fn sample(
        &self,
        clip: &str,
        seconds: f32,
        looped: bool,
    ) -> Result<Vec<SampledVertex>, String> {
        #[cfg(test)]
        let _timing = sample_timing::Guard::start();
        let clip = self
            .clips
            .get(clip)
            .ok_or_else(|| format!("glTF animation `{clip}` is missing"))?;
        let at = if looped {
            seconds.rem_euclid(clip.duration)
        } else {
            seconds.clamp(0.0, clip.duration)
        };
        let mut nodes = self.nodes.clone();
        for channel in &clip.channels {
            let value = channel.sample(at);
            let node = &mut nodes[channel.node];
            match channel.path {
                Path::Translation => node.translation = value.xyz(),
                Path::Rotation => {
                    node.rotation = Quat::from_xyzw(value.x, value.y, value.z, value.w).normalize()
                }
                Path::Scale => node.scale = value.xyz(),
            }
        }
        let mut globals = vec![None; nodes.len()];
        let mut skin = Vec::with_capacity(self.joints.len());
        for (joint, inverse) in self.joints.iter().zip(&self.inverse_bind) {
            skin.push(global(*joint, &nodes, &mut globals)? * *inverse);
        }
        Ok(self
            .source
            .iter()
            .map(|v| {
                let mut position = Vec3::ZERO;
                let mut normal = Vec3::ZERO;
                let mut total = 0.0;
                for i in 0..4 {
                    let weight = v.weights[i];
                    if weight > 0.0 {
                        let matrix = skin[v.joints[i] as usize];
                        position += matrix.transform_point3(v.position) * weight;
                        normal += matrix.transform_vector3(v.normal) * weight;
                        total += weight;
                    }
                }
                if total <= 0.0 {
                    position = v.position;
                    normal = v.normal;
                } else {
                    position /= total;
                    normal = (normal / total).try_normalize().unwrap_or(v.normal);
                }
                (position.to_array(), normal.to_array(), v.uv)
            })
            .collect())
    }
}

impl Channel {
    fn sample(&self, at: f32) -> Vec4 {
        let upper = self.times.partition_point(|&time| time <= at);
        if upper == 0 {
            return self.values[0];
        }
        if upper >= self.times.len() {
            return *self.values.last().unwrap();
        }
        let lower = upper - 1;
        let span = self.times[upper] - self.times[lower];
        let t = if span > 0.0 {
            (at - self.times[lower]) / span
        } else {
            0.0
        };
        if matches!(self.path, Path::Rotation) {
            let a = Quat::from_xyzw(
                self.values[lower].x,
                self.values[lower].y,
                self.values[lower].z,
                self.values[lower].w,
            );
            let b = Quat::from_xyzw(
                self.values[upper].x,
                self.values[upper].y,
                self.values[upper].z,
                self.values[upper].w,
            );
            Vec4::from_array(a.slerp(b, t).to_array())
        } else {
            self.values[lower].lerp(self.values[upper], t)
        }
    }
}

fn global(index: usize, nodes: &[Node], cache: &mut [Option<Mat4>]) -> Result<Mat4, String> {
    if let Some(value) = cache[index] {
        return Ok(value);
    }
    let node = nodes[index];
    let local = Mat4::from_scale_rotation_translation(node.scale, node.rotation, node.translation);
    let value = if let Some(parent) = node.parent {
        if parent == index {
            return Err("glTF node parents itself".into());
        }
        global(parent, nodes, cache)? * local
    } else {
        local
    };
    cache[index] = Some(value);
    Ok(value)
}

fn glb(bytes: &[u8]) -> Result<(Value, &[u8]), String> {
    if bytes.len() < 28
        || &bytes[..4] != b"glTF"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 2
    {
        return Err("asset is not binary glTF 2.0".into());
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if declared != bytes.len() {
        return Err("GLB declared length differs from bytes".into());
    }
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" || 20 + json_len + 8 > bytes.len() {
        return Err("GLB JSON chunk is malformed".into());
    }
    let bin_at = 20 + json_len;
    let bin_len = u32::from_le_bytes(bytes[bin_at..bin_at + 4].try_into().unwrap()) as usize;
    if &bytes[bin_at + 4..bin_at + 8] != b"BIN\0" || bin_at + 8 + bin_len > bytes.len() {
        return Err("GLB BIN chunk is malformed".into());
    }
    let doc =
        serde_json::from_slice(&bytes[20..20 + json_len]).map_err(|e| format!("GLB JSON: {e}"))?;
    Ok((doc, &bytes[bin_at + 8..bin_at + 8 + bin_len]))
}

fn array<'a>(doc: &'a Value, name: &str) -> Result<&'a Vec<Value>, String> {
    doc.get(name)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("glTF {name} is missing"))
}

fn index(value: Option<&Value>, name: &str) -> Result<usize, String> {
    value
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .ok_or_else(|| format!("glTF {name} is not an index"))
}

fn view_bytes<'a>(doc: &Value, bin: &'a [u8], view_index: usize) -> Result<&'a [u8], String> {
    let view = array(doc, "bufferViews")?
        .get(view_index)
        .ok_or("glTF bufferView is out of range")?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err("glTF external buffers are unsupported".into());
    }
    let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let len = view["byteLength"]
        .as_u64()
        .ok_or("glTF bufferView.byteLength is missing")? as usize;
    bin.get(start..start + len)
        .ok_or_else(|| "glTF bufferView exceeds BIN chunk".into())
}

fn accessor<'a>(
    doc: &Value,
    bin: &'a [u8],
    at: usize,
    lanes: usize,
) -> Result<(&'a [u8], usize, usize, u64), String> {
    let access = array(doc, "accessors")?
        .get(at)
        .ok_or("glTF accessor is out of range")?;
    let count = access["count"]
        .as_u64()
        .ok_or("glTF accessor.count is missing")? as usize;
    let component = access["componentType"]
        .as_u64()
        .ok_or("glTF accessor.componentType is missing")?;
    let width = match component {
        5123 => 2,
        5125 | 5126 => 4,
        _ => return Err("glTF accessor component type is unsupported".into()),
    };
    let view_index = index(access.get("bufferView"), "accessor.bufferView")?;
    let view = array(doc, "bufferViews")?.get(view_index).unwrap();
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or((width * lanes) as u64) as usize;
    if stride < width * lanes {
        return Err("glTF accessor stride is too short".into());
    }
    let offset = access
        .get("byteOffset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let bytes = view_bytes(doc, bin, view_index)?;
    let needed = offset + count.saturating_sub(1) * stride + width * lanes;
    Ok((
        bytes
            .get(offset..needed)
            .ok_or("glTF accessor exceeds bufferView")?,
        count,
        stride,
        component,
    ))
}

fn read_f32(doc: &Value, bin: &[u8], at: usize, lanes: usize) -> Result<Vec<f32>, String> {
    let (bytes, count, stride, component) = accessor(doc, bin, at, lanes)?;
    if component != 5126 {
        return Err("glTF accessor must contain f32".into());
    }
    let mut out = Vec::with_capacity(count * lanes);
    for row in 0..count {
        for lane in 0..lanes {
            let at = row * stride + lane * 4;
            out.push(f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()));
        }
    }
    Ok(out)
}

fn read_u16(doc: &Value, bin: &[u8], at: usize, lanes: usize) -> Result<Vec<u16>, String> {
    let (bytes, count, stride, component) = accessor(doc, bin, at, lanes)?;
    if component != 5123 {
        return Err("glTF accessor must contain u16".into());
    }
    let mut out = Vec::with_capacity(count * lanes);
    for row in 0..count {
        for lane in 0..lanes {
            let at = row * stride + lane * 2;
            out.push(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()));
        }
    }
    Ok(out)
}

fn read_indices(doc: &Value, bin: &[u8], at: usize) -> Result<Vec<u32>, String> {
    let (bytes, count, stride, component) = accessor(doc, bin, at, 1)?;
    let width = if component == 5123 {
        2
    } else if component == 5125 {
        4
    } else {
        return Err("glTF indices must be u16 or u32".into());
    };
    Ok((0..count)
        .map(|row| {
            let at = row * stride;
            if width == 2 {
                u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as u32
            } else {
                u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
            }
        })
        .collect())
}

fn vec3(value: Option<&Value>, default: Vec3) -> Vec3 {
    value
        .and_then(Value::as_array)
        .filter(|a| a.len() == 3)
        .map_or(default, |a| {
            Vec3::new(
                a[0].as_f64().unwrap_or_default() as f32,
                a[1].as_f64().unwrap_or_default() as f32,
                a[2].as_f64().unwrap_or_default() as f32,
            )
        })
}

fn quat(value: Option<&Value>, default: Quat) -> Quat {
    value
        .and_then(Value::as_array)
        .filter(|a| a.len() == 4)
        .map_or(default, |a| {
            Quat::from_xyzw(
                a[0].as_f64().unwrap_or_default() as f32,
                a[1].as_f64().unwrap_or_default() as f32,
                a[2].as_f64().unwrap_or_default() as f32,
                a[3].as_f64().unwrap_or(1.0) as f32,
            )
            .normalize()
        })
}

fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("glTF PNG: {e}"))?;
    let mut raw = vec![0; reader.output_buffer_size().ok_or("glTF PNG is too large")?];
    let info = reader
        .next_frame(&mut raw)
        .map_err(|e| format!("glTF PNG: {e}"))?;
    raw.truncate(info.buffer_size());
    let mut rgba = Vec::with_capacity(info.width as usize * info.height as usize * 4);
    match info.color_type {
        png::ColorType::Rgba => rgba = raw,
        png::ColorType::Rgb => {
            for p in raw.chunks_exact(3) {
                rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &v in &raw {
                rgba.extend_from_slice(&[v, v, v, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in raw.chunks_exact(2) {
                rgba.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
            }
        }
        png::ColorType::Indexed => return Err("glTF PNG palette was not expanded".into()),
    }
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::Model;

    const FOX: &[u8] = include_bytes!("../../games/lanterns/assets/Fox.glb");

    #[test]
    fn fox_decodes_texture_skin_and_three_named_clips() {
        let fox = Model::parse(FOX).unwrap();
        assert_eq!(fox.source.len(), 1728);
        assert_eq!(fox.indices.len(), 1728);
        assert_eq!((fox.image.width, fox.image.height), (1024, 1024));
        assert_eq!(
            fox.clip_names().collect::<Vec<_>>(),
            ["Run", "Survey", "Walk"]
        );
        let survey = fox.sample("Survey", 0.0, true).unwrap();
        let run = fox.sample("Run", 0.5, true).unwrap();
        assert!(survey.iter().all(|v| v.0.iter().all(|n| n.is_finite())));
        assert!(survey.iter().zip(&run).any(|(a, b)| a.0 != b.0));
    }
}

// Opt-in test instrumentation: no production fields or timing work.
#[cfg(test)]
pub(crate) mod sample_timing {
    use std::{cell::Cell, time::Instant};
    thread_local! {
        pub static NANOS: Cell<Option<u128>> = const { Cell::new(None) };
    }
    pub struct Guard(Option<Instant>);
    impl Guard {
        pub fn start() -> Self {
            Self(NANOS.with(|n| n.get().map(|_| Instant::now())))
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(start) = self.0 {
                NANOS.with(|n| n.set(Some(n.get().unwrap() + start.elapsed().as_nanos())));
            }
        }
    }
}
