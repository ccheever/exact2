// Model shadow passes bind the cascade camera at group 1, so their shared model
// source replaces shadow_sample.wgsl with this stub instead of sampling maps.
fn sun_visibility(p: vec3<f32>, n: vec3<f32>) -> f32 { return 1.0; }
fn light_visibility(at: u32, world: vec3<f32>, n: vec3<f32>, l: vec3<f32>) -> f32 { return 1.0; }
