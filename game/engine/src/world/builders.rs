use crate::{Entity, Material, Mesh, Sprite, Transform, Vec3, World};

/// A spawned prop's literal setters. Defaults match `(Transform, Mesh, Material)`.
/// `prop` spawns immediately; setters mutate only its authored rows.
pub struct PropBuilder<'w> {
    world: &'w World,
    entity: Entity,
}
impl PropBuilder<'_> {
    /// Local position in metres.
    pub fn at(self, x: f32, y: f32, z: f32) -> Self {
        self.world.require_mut::<Transform>(self.entity).position = Vec3::new(x, y, z);
        self
    }
    /// sRGB hex colour, converted to the material's linear RGBA.
    pub fn color(self, hex: &str) -> Self {
        *self.world.require_mut::<Material>(self.entity) = color(hex);
        self
    }
    /// The spawned entity, for adding game components or referring to it later.
    pub fn entity(self) -> Entity {
        self.entity
    }
}

/// A spawned sprite's literal setters. Defaults match `(Transform, Sprite)`.
pub struct SpriteBuilder<'w> {
    world: &'w World,
    entity: Entity,
}
impl SpriteBuilder<'_> {
    /// Local XY position in world units; Z stays zero.
    pub fn at(self, x: f32, y: f32) -> Self {
        self.world.require_mut::<Transform>(self.entity).position = Vec3::new(x, y, 0.);
        self
    }
    /// The spawned entity, for adding components or referring to it later.
    pub fn entity(self) -> Entity {
        self.entity
    }
}

impl World {
    /// Spawn a named prop with default Transform and Material, then set literals.
    /// `.at(x, y, z).color("#4d6669")` produces the same rows as the corresponding tuple.
    pub fn prop(&mut self, name: impl AsRef<str>, mesh: Mesh) -> PropBuilder<'_> {
        let entity = self.spawn_named(name, (Transform::default(), mesh, Material::default()));
        PropBuilder {
            world: self,
            entity,
        }
    }
    /// Spawn a named sprite with default Transform, then set its XY position.
    /// The texture must be declared in Game::ASSETS and baked with game.assets.
    pub fn sprite(
        &mut self,
        name: impl AsRef<str>,
        texture: impl Into<String>,
        size: [f32; 2],
    ) -> SpriteBuilder<'_> {
        let entity = self.spawn_named(name, (Transform::default(), Sprite::new(texture, size)));
        SpriteBuilder {
            world: self,
            entity,
        }
    }
}

fn color(hex: &str) -> Material {
    let parsed = hex
        .starts_with('#')
        .then(|| exact_color::parse(hex))
        .flatten();
    let Some(exact_color::Parsed::Legacy(c)) = parsed else {
        panic!("prop.color: expected #rgb, #rgba, #rrggbb or #rrggbbaa, got `{hex}`");
    };
    let [r, g, b] =
        [c.r, c.g, c.b].map(|v| exact_color::srgb_to_linear(f64::from(v) / 255.) as f32);
    Material {
        color: [r, g, b, c.a as f32],
        ..Material::default()
    }
}
