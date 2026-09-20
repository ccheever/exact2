use exact_world::*;

pub struct Fails;
#[derive(Default, Args)]
pub struct Options {
    restart: bool,
}
#[derive(Default, Component)]
struct Marker;
impl Game for Fails {
    const ID: &'static str = "tick-three-failure";
    type Args = Options;
    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        w.register::<Marker>()?;
        Ok(())
    }
    fn setup(w: &mut World, _: &Options) -> Result<(), DataError> {
        let parent = w.spawn_named("owner", Marker)?;
        let child = w.spawn_named("child", Marker)?;
        w.set_parent(child, Some(parent))?;
        w.publish("ticks", 0u32)?;
        Ok(())
    }
    fn tick(w: &mut World, _: &Input, _: &Options) -> Result<(), DataError> {
        if w.tick() == 2 {
            w.despawn(
                w.named("owner")
                    .ok_or_else(|| DataError::new("missing owner"))?,
            )?;
            w.emit("must not escape")?;
            return Err(DataError::new("fixture tick refused"));
        }
        w.publish("ticks", (w.tick() + 1) as u32)?;
        Ok(())
    }
}
