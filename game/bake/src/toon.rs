//! Optional toon data uses glTF extras, not a required glTF extension.
use exact_game::asset::MeshOutline;
use serde_json::Value;

fn extras(raw: &gltf::json::Extras) -> Result<Value, String> {
    raw.as_ref()
        .map(|v| serde_json::from_str(v.get()).map_err(|e| e.to_string()))
        .unwrap_or(Ok(Value::Null))
}
fn number(v: &Value, key: &str) -> Result<f32, String> {
    let x = v
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("toon {key}: expected number"))? as f32;
    if !x.is_finite() || x < 0. {
        return Err(format!("toon {key}: expected finite nonnegative number"));
    }
    Ok(x)
}
pub fn rim(m: &gltf::Material<'_>) -> Result<f32, String> {
    let e = extras(m.extras())?;
    e.get("toon")
        .or_else(|| e.get("castle"))
        .map(|v| number(v, "rim"))
        .unwrap_or(Ok(0.))
}
pub fn outline(
    p: &gltf::Primitive<'_>,
    buffers: &[gltf::buffer::Data],
) -> Result<Option<MeshOutline>, String> {
    let e = extras(p.extras())?;
    let settings = e.get("outline").or_else(|| e.get("castle_outline"));
    let normals = p.get(&gltf::Semantic::Extras("OUTLINE_NORMAL".into()));
    let reach = p.get(&gltf::Semantic::Extras("OUTLINE_REACH".into()));
    if settings.is_none() && normals.is_none() && reach.is_none() {
        return Ok(None);
    }
    let s = settings.ok_or("_OUTLINE_NORMAL/_OUTLINE_REACH require outline extras")?;
    let normals = normals.ok_or("outline requires _OUTLINE_NORMAL")?;
    let reach = reach.ok_or("outline requires _OUTLINE_REACH")?;
    use gltf::accessor::{DataType, Dimensions, Iter};
    for (a, dims, name) in [
        (&normals, Dimensions::Vec3, "_OUTLINE_NORMAL"),
        (&reach, Dimensions::Scalar, "_OUTLINE_REACH"),
    ] {
        if a.data_type() != DataType::F32 || a.dimensions() != dims || a.normalized() {
            return Err(format!("{name}: expected unnormalized FLOAT {dims:?}"));
        }
    }
    let count = p
        .get(&gltf::Semantic::Positions)
        .ok_or("missing POSITION")?
        .count();
    for (a, name) in [(&normals, "_OUTLINE_NORMAL"), (&reach, "_OUTLINE_REACH")] {
        if a.count() != count {
            return Err(format!(
                "{name}: count {} must equal POSITION count {count}",
                a.count()
            ));
        }
    }
    match p.material().alpha_mode() {
        gltf::material::AlphaMode::Mask => {
            return Err("outline with alpha-tested material (MASK) is unsupported".into())
        }
        gltf::material::AlphaMode::Blend => {
            return Err("outline with blended material (BLEND) is unsupported".into())
        }
        _ => {}
    }
    let buffer = |b: gltf::Buffer<'_>| Some(buffers[b.index()].0.as_slice());
    let color: [f32; 3] =
        serde_json::from_value(s.get("color").cloned().ok_or("outline requires color")?)
            .map_err(|e| format!("outline color: {e}"))?;
    Ok(Some(MeshOutline {
        width: number(s, "width")?,
        min_pixels: number(s, "minPixels")?,
        color,
        normals: Iter::<[f32; 3]>::new(normals, buffer)
            .ok_or("invalid _OUTLINE_NORMAL accessor")?
            .flatten()
            .collect(),
        reach: Iter::<f32>::new(reach, buffer)
            .ok_or("invalid _OUTLINE_REACH accessor")?
            .collect(),
    }))
}
pub fn same_outline(a: Option<&MeshOutline>, b: Option<&MeshOutline>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.width == b.width && a.min_pixels == b.min_pixels && a.color == b.color
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn source() -> Value {
        json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":84}],
            "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36},{"buffer":0,"byteOffset":72,"byteLength":12}],
            "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":2,"componentType":5126,"count":3,"type":"SCALAR"}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0,"_OUTLINE_NORMAL":1,"_OUTLINE_REACH":2},"extras":{"outline":{"width":0.02,"minPixels":1.1,"color":[0.1,0.2,0.3]}}}]}],
            "materials":[{"extras":{"toon":{"rim":0.35}}}]})
    }
    fn read(v: &Value) -> Result<exact_game::asset::MeshData, String> {
        let g = gltf::Gltf::from_slice(v.to_string().as_bytes()).map_err(|e| e.to_string())?;
        let values = [
            0f32, 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 1., 0., 0., 1., 0.1, 0.2, 0.3,
        ];
        crate::geometry::primitive(
            &g.meshes().next().unwrap().primitives().next().unwrap(),
            &[gltf::buffer::Data(
                values.into_iter().flat_map(f32::to_le_bytes).collect(),
            )],
            0,
        )
    }
    #[test]
    fn toon_attributes_extras_and_binary_round_trip() {
        for (outline_key, rim_key) in [("outline", "toon"), ("castle_outline", "castle")] {
            let mut v = source();
            let e = &mut v["meshes"][0]["primitives"][0]["extras"];
            let settings = e.as_object_mut().unwrap().remove("outline").unwrap();
            e[outline_key] = settings;
            v["materials"][0]["extras"] = json!({rim_key:{"rim":0.35}});
            let mesh = read(&v).unwrap();
            let o = mesh.outline.as_ref().unwrap();
            assert_eq!(
                (o.width, o.min_pixels, o.color),
                (0.02, 1.1, [0.1, 0.2, 0.3])
            );
            assert_eq!(o.reach, [0.1, 0.2, 0.3]);
            assert_eq!(o.normals, [0., 0., 1.].repeat(3));
            let g = gltf::Gltf::from_slice(v.to_string().as_bytes()).unwrap();
            let material = exact_game::asset::MaterialData {
                rim: rim(&g.materials().next().unwrap()).unwrap(),
                ..Default::default()
            };
            let model = exact_game::asset::Model {
                meshes: vec![mesh],
                materials: vec![material],
                nodes: vec![exact_game::asset::Node {
                    mesh: Some(0),
                    ..Default::default()
                }],
                bounds: [0., 0., 0., 1., 1., 0.],
                ..Default::default()
            };
            model.validate().unwrap();
            let decoded: exact_game::asset::Model =
                exact_game::bin::from_slice(&exact_game::bin::to_vec(&model)).unwrap();
            assert_eq!(decoded.materials[0].rim, 0.35);
            assert_eq!(
                decoded.meshes[0].outline.as_ref().unwrap().reach,
                vec![0.1, 0.2, 0.3]
            );
        }
    }
    #[test]
    fn outline_accessor_counts_match_each_primitive_before_merging() {
        for (index, name) in [(1, "_OUTLINE_NORMAL"), (2, "_OUTLINE_REACH")] {
            for count in [2, 4] {
                let mut v = source();
                v["accessors"][index]["count"] = json!(count);
                let error = read(&v).unwrap_err();
                assert!(
                    error.contains(name) && error.contains("POSITION count 3"),
                    "{error}"
                );
            }
        }
    }
    #[test]
    fn outlined_alpha_materials_refuse_at_import_and_model_validation() {
        use exact_game::asset::*;
        for (mode, alpha, label) in [
            ("MASK", AlphaMode::Mask, "alpha-tested"),
            ("BLEND", AlphaMode::Blend, "blended"),
        ] {
            let mut v = source();
            v["meshes"][0]["primitives"][0]["material"] = json!(0);
            v["materials"][0]["alphaMode"] = json!(mode);
            assert!(read(&v).unwrap_err().contains(label));
            let model = Model {
                meshes: vec![read(&source()).unwrap()],
                materials: vec![MaterialData {
                    alpha_mode: alpha,
                    ..Default::default()
                }],
                ..Default::default()
            };
            let error = model.validate().unwrap_err();
            assert!(error.contains("mesh 0") && error.contains(label), "{error}");
        }
    }
    #[test]
    fn unknown_semantics_and_incomplete_or_malformed_outlines_refuse_by_name() {
        let mut v = source();
        v["meshes"][0]["primitives"][0]["attributes"]["_OTHER"] = json!(1);
        assert!(read(&v).unwrap_err().contains("OTHER"));
        let mut v = source();
        v["meshes"][0]["primitives"][0]["attributes"]
            .as_object_mut()
            .unwrap()
            .remove("_OUTLINE_REACH");
        assert!(read(&v).unwrap_err().contains("_OUTLINE_REACH"));
        let mut v = source();
        v["accessors"][2]["type"] = json!("VEC3");
        v["accessors"][2]["count"] = json!(1);
        assert!(read(&v).unwrap_err().contains("_OUTLINE_REACH"));
        let mut v = source();
        v["meshes"][0]["primitives"][0]["extras"]["outline"]["width"] = json!(-1);
        assert!(read(&v).unwrap_err().contains("width"));
        let mut v = source();
        v["meshes"][0]["primitives"][0]["extras"] = json!({});
        assert!(read(&v).unwrap_err().contains("outline extras"));
    }
}
