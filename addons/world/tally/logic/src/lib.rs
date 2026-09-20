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
    fn setup(w: &mut World, args: &Options) -> Result<(), DataError> {
        w.reseed(args.seed as u64);
        w.spawn_named("hand", Owner)?;
        w.spawn_named("pile", Owner)?;
        w.spawn_named("held", Owner)?;
        w.insert_resource(Round::default())?;
        let mut deck = Vec::new();
        for i in 0..12 {
            deck.push(w.spawn_named(format!("card-{i}"), Card { value: i + 1 })?);
        }
        w.resource_mut::<Round>().deck = deck;
        reset(w)
    }
    fn tick(w: &mut World, input: &Input, _: &Options) -> Result<(), DataError> {
        if input.pressed("reset") {
            reset(w)?;
        }
        if !w.resource::<Round>().over {
            if input.pressed("draw") {
                let card = {
                    let mut r = w.resource_mut::<Round>();
                    let card = r.deck[r.drawn as usize];
                    r.drawn += 1;
                    card
                };
                w.set_parent(card, w.named("hand"))?;
            }
            let total: u32 = hand(w)?.iter().sum();
            if total > 21 {
                w.resource_mut::<Round>().over = true;
            } else if input.pressed("hold") {
                w.resource_mut::<Round>().score += total;
                let cards: Vec<_> = w.children("hand").take(12).collect();
                for e in cards {
                    w.set_parent(e, w.named("held"))?;
                }
            }
            if w.resource::<Round>().drawn == 12 {
                w.resource_mut::<Round>().over = true;
            }
        }
        let round = w.resource::<Round>();
        w.publish("score", round.score)?;
        w.publish(
            "hand",
            Published::List(
                hand(w)?
                    .into_iter()
                    .map(|v| Published::Number(v as f64))
                    .collect(),
            ),
        )?;
        w.publish("pile_count", 12 - round.drawn)?;
        w.publish("over", round.over)?;
        w.publish("ticks", (w.tick() + 1) as f64)?;
        Ok(())
    }
}
fn hand(w: &World) -> Result<Vec<u32>, DataError> {
    // Hand presentation is slot-ordered; Round.deck alone defines draw order.
    let mut values = Vec::new();
    for e in w.children("hand").take(13) {
        if values.len() == 12 {
            return Err(DataError::new("Tally hand exceeds twelve cards"));
        }
        values.push(
            w.get::<Card>(e)
                .ok_or_else(|| DataError::new("Tally hand child is not a card"))?
                .value,
        );
    }
    Ok(values)
}

fn reset(w: &mut World) -> Result<(), DataError> {
    let mut deck = w.resource::<Round>().deck.clone();
    for i in (1..deck.len()).rev() {
        let j = w.rng().next_u32() as usize % (i + 1);
        deck.swap(i, j);
    }
    for &e in &deck {
        w.set_parent(e, w.named("pile"))?;
    }
    *w.resource_mut::<Round>() = Round {
        deck,
        ..Default::default()
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hand_uses_children_in_slot_order_after_interleaving_and_reparenting() {
        let mut world = World::new(60, 7);
        world.register::<Parent>().unwrap();
        world.register::<Card>().unwrap();
        world.register::<Owner>().unwrap();
        world.spawn_named("hand", Owner).unwrap();
        world.spawn_named("pile", Owner).unwrap();
        let mut cards = Vec::new();
        for value in 1..=12 {
            // Unrelated cards interleave the actual hand in the same column.
            for _ in 0..1000 {
                world.spawn(Card { value: 999 }).unwrap();
            }
            cards.push(world.spawn(Card { value }).unwrap());
        }
        for &card in cards.iter().rev() {
            world.set_parent(card, world.named("hand")).unwrap();
        }
        assert_eq!(hand(&world).unwrap(), (1..=12).collect::<Vec<_>>());
        world.set_parent(cards[4], world.named("pile")).unwrap();
        assert_eq!(
            hand(&world).unwrap(),
            vec![1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12]
        );
        world.set_parent(cards[4], world.named("hand")).unwrap();
        assert_eq!(hand(&world).unwrap(), (1..=12).collect::<Vec<_>>());
        let extra = world.spawn(Card { value: 13 }).unwrap();
        world.set_parent(extra, world.named("hand")).unwrap();
        assert!(hand(&world)
            .unwrap_err()
            .to_string()
            .contains("twelve cards"));
    }
}
