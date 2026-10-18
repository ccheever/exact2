// Only opaque models without coverage fading: early depth must never retain a
// fragment that the material would discard. Appended on devices with the feature.
@fragment @early_depth_test(force) fn model_fs_early(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
    return finish(model_shade(input,front,1.0));
}
@fragment @early_depth_test(force) fn model_fs_shadow_early(input:ModelVarying,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
    return finish(model_shade(input,front,sun_visibility(input.world,normalize(input.normal))));
}
