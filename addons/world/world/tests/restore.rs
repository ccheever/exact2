//! Diagnostic restore timing, using unchanged inputs on both experiment revisions.
use exact_world::*;
use std::time::Instant;
#[derive(Default, Component)]
struct Position(u32);
#[derive(Default, Component)]
struct Controller(u32);
#[derive(Default, Component)]
struct Tag;
struct Large;
impl Game for Large {
    const ID: &'static str = "lean-restore";
    type Args = ();
    fn register(w: &mut World, _: args::SetupArgs<'_, ()>) -> Result<(), DataError> {
        w.register::<Position>()?
            .register::<Controller>()?
            .register::<Tag>()?;
        Ok(())
    }
    fn tick(_: &mut World, _: &Input, _: &()) -> Result<(), DataError> {
        Ok(())
    }
    fn setup(w: &mut World, _: &()) -> Result<(), DataError> {
        for _ in 0..MAX_ENTITIES {
            w.spawn(())?;
        }
        for i in (0..MAX_ENTITIES).rev() {
            let e = w.entity_at(i).unwrap();
            if i % 2 == 0 {
                w.insert(e, Position(i as u32))?;
            }
            if i % 16 == 0 {
                w.insert(e, Controller(i as u32))?;
            }
            if i % 64 == 0 {
                w.insert(e, Tag)?;
            }
        }
        w.work("assets", Work::Pending)?;
        Ok(())
    }
}
#[test]
#[ignore = "20 release-profile restore measurements"]
fn restore_median_20() {
    let mut sim = Sim::<Large>::new(()).unwrap();
    let bytes = sim.save().unwrap();
    sim.restore(&bytes).unwrap();
    let mut samples = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        sim.restore(&bytes).unwrap();
        samples.push(start.elapsed().as_nanos());
    }
    samples.sort_unstable();
    assert_eq!(sim.save().unwrap(), bytes);
    assert_eq!(sim.world().len(), MAX_ENTITIES);
    assert_eq!(sim.world().get::<Controller>("#199984").unwrap().0, 199984);
    println!(
        "restore: bytes={} median_ns={} samples={samples:?}",
        bytes.len(),
        (samples[9] + samples[10]) / 2
    );
}
