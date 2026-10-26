use super::*;

// The engine's own scene components restore in any process, however late a
// game first spawns one. Registration is not state; executor-linked
// components (animation, sockets, audio) still register where they install.
pub(crate) fn register(world: &mut World) {
    crate::input::register_resources(world.register_resource::<crate::Shadows>());
    world
        .register::<Transform>()
        .register::<Parent>()
        .register::<Mesh>()
        .register::<crate::Material>()
        .register::<Camera>()
        .register::<DirectionalLight>()
        .register::<PointLight>()
        .register::<SpotLight>()
        .register::<LightShadows>()
        .register::<ViewModel>()
        .register::<Opacity>()
        .register::<NodeMaterials>()
        .register::<MaterialOverrides>()
        .register::<ModelLod>()
        .register::<crate::ParticleLook>()
        .register::<crate::NodeOffsets>()
        .register::<crate::Offset>()
        .register::<crate::Opacity>()
        .register::<crate::Tint>()
        .register::<MouseLook>()
        .register::<Visible>()
        .register::<Ambient>()
        .register::<Follow>()
        .register::<Glow>()
        .register::<Lit>()
        .register::<crate::emitter::WorldSpace>();
}
