use super::*;

#[derive(Kind)]
pub(super) struct Hero {
    pub character: Character,
    #[read]
    pub transform: Transform,
}
#[derive(Kind)]
pub(super) struct Stride {
    pub player: Player,
}
#[derive(Kind)]
pub(super) struct Fox {
    pub animation: Animation,
}
#[derive(Kind)]
pub(super) struct Pose {
    pub transform: Transform,
}
#[derive(Kind)]
pub(super) struct Lamp {
    #[read]
    pub transform: Transform,
    pub lantern: Lantern,
}
#[derive(Kind)]
pub(super) struct Bulb {
    pub material: Material,
    pub light: PointLight,
}

#[derive(Clone, Copy)]
pub(super) struct Actors {
    pub hero: Id<Hero>,
    pub stride: Id<Stride>,
    pub fox: Id<Fox>,
    pub fox_pose: Id<Pose>,
    pub sun: Id<Pose>,
}

// These are derived bindings inside existing saved records. They add no bytes
// to saves, hashes or agent JSON; first use after load rebuilds them once.
pub(super) fn actors(world: &World) -> Actors {
    if let Some(ids) = world.resource::<Session>().actors {
        return ids;
    }
    let fox = world.the::<Fox>();
    let ids = Actors {
        hero: world.the::<Hero>(),
        stride: world.the::<Stride>(),
        fox,
        fox_pose: world.bind(fox).unwrap_or_else(|e| panic!("{e}")),
        sun: world.bind("sun").unwrap_or_else(|e| panic!("{e}")),
    };
    for mut row in world.rows_mut::<Lamp>() {
        row.lantern.bulb = world
            .child(row.id, "bulb")
            .unwrap_or_else(|e| panic!("{e}"));
    }
    world.resource_mut::<Session>().actors = Some(ids);
    ids
}
