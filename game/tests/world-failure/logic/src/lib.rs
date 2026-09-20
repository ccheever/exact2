use exact_world::*;

pub struct Fails;
impl Game for Fails {
    const ID: &'static str = "tick-three-failure";
    type Args = ();
    fn setup(w: &mut World, _: &()) -> Result<(), DataError> {
        w.publish("ticks", 0u32)?;
        Ok(())
    }
    fn tick(w: &mut World, _: &Input, _: &()) -> Result<(), DataError> {
        if w.tick() == 2 {
            return Err(DataError::new("fixture tick refused"));
        }
        w.publish("ticks", (w.tick() + 1) as u32)?;
        Ok(())
    }
}
