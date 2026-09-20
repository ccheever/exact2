use exact_world::*;

#[derive(Default, Args)]
pub struct Options {
    pub seed: u32,
}
#[derive(Default, Component)]
pub struct Card {
    pub value: u32,
}
#[derive(Default, Component)]
pub struct Owner;
#[derive(Default, Resource)]
pub struct Round {
    pub deck: Vec<Entity>,
    pub drawn: u32,
    pub score: u32,
    pub over: bool,
}

pub struct Tally;
impl Game for Tally {
    const ID: &'static str = "tally";
    type Args = Options;
    const ACTIONS: &'static [Action] = &[
        Action::button("draw", &["KeyD"]),
        Action::button("hold", &["KeyH"]),
        Action::button("reset", &["KeyR"]),
    ];
    fn register(w: &mut World, _: args::SetupArgs<'_, Options>) -> Result<(), DataError> {
        w.register::<Parent>()?;
        w.register::<Card>()?;
        w.register::<Owner>()?;
        w.register_resource::<Round>()?;
        Ok(())
    }
    fn setup(w: &mut World, args: &Options) {
        w.reseed(args.seed as u64);
        w.spawn_named("hand", Owner).unwrap();
        w.spawn_named("pile", Owner).unwrap();
        w.spawn_named("held", Owner).unwrap();
        w.insert_resource(Round::default()).unwrap();
        let mut deck = Vec::new();
        for i in 0..12 {
            deck.push(
                w.spawn_named(format!("card-{i}"), Card { value: i + 1 })
                    .unwrap(),
            );
        }
        w.resource_mut::<Round>().deck = deck;
        reset(w);
    }
    fn tick(w: &mut World, input: &Input, _: &Options) {
        if input.pressed("reset") {
            reset(w);
        }
        if !w.resource::<Round>().over {
            if input.pressed("draw") {
                let card = {
                    let mut r = w.resource_mut::<Round>();
                    let card = r.deck[r.drawn as usize];
                    r.drawn += 1;
                    card
                };
                w.set_parent(card, w.named("hand")).unwrap();
            }
            let total: u32 = hand(w).iter().sum();
            if total > 21 {
                w.resource_mut::<Round>().over = true;
            } else if input.pressed("hold") {
                w.resource_mut::<Round>().score += total;
                let cards: Vec<_> = w
                    .query::<&Card>()
                    .iter()
                    .filter(|(e, _)| {
                        w.get::<Parent>(*e)
                            .is_some_and(|p| Some(p.entity()) == w.named("hand"))
                    })
                    .map(|(e, _)| e)
                    .collect();
                for e in cards {
                    w.set_parent(e, w.named("held")).unwrap();
                }
            }
            if w.resource::<Round>().drawn == 12 {
                w.resource_mut::<Round>().over = true;
            }
        }
        let round = w.resource::<Round>();
        w.publish("score", round.score).unwrap();
        w.publish(
            "hand",
            Published::List(
                hand(w)
                    .into_iter()
                    .map(|v| Published::Number(v as f64))
                    .collect(),
            ),
        )
        .unwrap();
        w.publish("pile_count", 12 - round.drawn).unwrap();
        w.publish("over", round.over).unwrap();
        w.publish("ticks", (w.tick() + 1) as f64).unwrap();
    }
}
fn hand(w: &World) -> Vec<u32> {
    w.query::<&Card>()
        .iter()
        .filter(|(e, _)| {
            w.get::<Parent>(*e)
                .is_some_and(|p| Some(p.entity()) == w.named("hand"))
        })
        .map(|(_, c)| c.value)
        .collect()
}
fn reset(w: &mut World) {
    let mut deck = w.resource::<Round>().deck.clone();
    for i in (1..deck.len()).rev() {
        let j = w.rng().next_u32() as usize % (i + 1);
        deck.swap(i, j);
    }
    for &e in &deck {
        w.set_parent(e, w.named("pile")).unwrap();
    }
    *w.resource_mut::<Round>() = Round {
        deck,
        ..Default::default()
    };
}
