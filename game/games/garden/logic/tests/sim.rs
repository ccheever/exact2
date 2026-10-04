use exact_game::{Material, Sim, Transform, Visible};
use garden_logic::crops::{kind_of, CROPS};
use garden_logic::farm::Farm;
use garden_logic::garden::{now_ms, Census, Fruit, GardenClock, Plant, Schedule, Weather};
use garden_logic::shop::Shop;
use garden_logic::{census_of, Garden, Options};

fn new(seed: u64) -> Sim<Garden> {
    Sim::<Garden>::new(Options {
        seed,
        ..Options::default()
    })
    .unwrap()
}

/// A HUD command, as Contract's `postMessage("world", cmd)` sends it.
fn send(game: &mut Sim<Garden>, cmd: &str) {
    game.post(cmd);
    game.run(100.0);
}

fn sheckles(game: &Sim<Garden>) -> u64 {
    game.world().resource::<Farm>().sheckles
}

#[test]
fn plant_grow_harvest_sell() {
    let mut game = new(7);
    // The starting hand: one carrot seed, 20 sheckles; the player stands on tile 0,0.
    assert_eq!(sheckles(&game), 20);
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(game.world().resource::<Census>().plants, 1);
    assert_eq!(game.world().resource::<Farm>().seeds[0], 0);
    let prompt = game.world().published("prompt").unwrap();
    assert!(
        prompt.text().starts_with("Carrot growing"),
        "{}",
        prompt.text()
    );
    // Not ripe yet: E harvests nothing.
    game.tap("KeyE");
    game.run(100.0);
    assert!(game.world().resource::<Farm>().bag.is_empty());
    game.run(20_000.0);
    assert_eq!(game.world().resource::<Census>().ripe, 1);
    assert_eq!(
        game.world().published("prompt").unwrap().text(),
        "E: harvest 1 Carrot"
    );
    game.tap("KeyE");
    game.run(100.0);
    let bag = game.world().resource::<Farm>().bag.clone();
    assert_eq!(bag.len(), 1);
    // A carrot is single-harvest: the plant is gone, the tile free.
    assert_eq!(game.world().resource::<Census>().plants, 0);
    assert!(game.world().resource::<Farm>().at([0, 0]).is_none());
    let value = bag[0].value();
    send(&mut game, "sell all");
    assert_eq!(sheckles(&game), 20 + value);
    assert_eq!(
        game.world().published("bag_count").unwrap().as_number(),
        Some(0.0)
    );
}

#[test]
fn buy_from_the_shop_and_regrow() {
    let mut game = new(7);
    send(&mut game, "buy carrot");
    assert_eq!(sheckles(&game), 10);
    assert_eq!(game.world().resource::<Farm>().seeds[0], 2);
    send(&mut game, "buy strawberry");
    assert!(
        game.world().resource::<Farm>().last.contains("costs 50"),
        "{}",
        game.world().resource::<Farm>().last
    );
    game.world_mut().resource_mut::<Farm>().sheckles = 1000;
    let berry = kind_of("strawberry").unwrap();
    let stock = game.world().resource::<Shop>().stock[berry as usize];
    send(&mut game, "buy strawberry");
    if stock == 0 {
        assert_eq!(game.world().resource::<Farm>().seeds[berry as usize], 0);
        return;
    }
    assert_eq!(game.world().resource::<Farm>().seeds[berry as usize], 1);
    send(&mut game, "equip strawberry");
    assert_eq!(game.world().resource::<Farm>().held, Some(berry));
    game.tap("KeyE");
    game.run(40_000.0 + 30_000.0 + 200.0);
    let ripe = game.world().resource::<Census>().ripe;
    assert_eq!(ripe, 4, "four slots ripen together");
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().bag.len(), 4);
    // A regrowing crop stays and sets new fruit.
    assert_eq!(game.world().resource::<Census>().plants, 1);
    assert_eq!(game.world().resource::<Census>().fruit, 4);
    assert_eq!(game.world().resource::<Census>().ripe, 0);
    game.run(30_100.0);
    assert_eq!(game.world().resource::<Census>().ripe, 4);
}

#[test]
fn shop_restocks_every_five_minutes() {
    let mut game = new(3);
    assert_eq!(game.world().resource::<Shop>().restocks, 1);
    assert_eq!(game.world().published("restock_in").unwrap().text(), "5:00");
    game.run(60_100.0);
    assert_eq!(game.world().published("restock_in").unwrap().text(), "4:00");
    game.run(240_100.0);
    assert_eq!(game.world().resource::<Shop>().restocks, 2);
    // Carrots always appear.
    assert!(game.world().resource::<Shop>().stock[0] > 0);
}

#[test]
fn same_seed_same_mutations_other_seed_other_mutations() {
    let grow = |seed| {
        let mut game = new(seed);
        send(&mut game, "fill 400");
        game.run(15.0 * 60_000.0);
        (census_of(game.world()), game.world().hash())
    };
    let (a, ha) = grow(11);
    let (b, hb) = grow(11);
    let (c, _) = grow(12);
    assert_eq!(a, b);
    assert_eq!(ha, hb);
    assert_ne!(a, c);
    let mutated = a
        .iter()
        .filter(|l| l.starts_with("fruit") && !l.contains(" 0 "))
        .count();
    assert!(
        mutated > 0,
        "fifteen minutes of 400 plants mutates something"
    );
}

/// The strongest determinism property: being away for a span and playing
/// through it produce the same garden — the same events in the same order,
/// drawing the same random numbers.
#[test]
fn away_is_the_same_as_playing_through() {
    let mut live = new(5);
    send(&mut live, "fill 300");
    live.tap("KeyQ");
    live.run(100.0);
    let mut away = new(5);
    send(&mut away, "fill 300");
    away.tap("KeyQ");
    away.run(100.0);
    live.run(20.0 * 60_000.0);
    let target = now_ms(live.world());
    let gap = target - now_ms(away.world()) - 1000 / 30;
    send(&mut away, &format!("away {}", gap / 1000));
    // `away` takes whole seconds; finish the span tick by tick.
    while now_ms(away.world()) < target {
        away.run(1000.0 / 30.0);
    }
    assert_eq!(now_ms(live.world()), now_ms(away.world()));
    assert_eq!(census_of(live.world()), census_of(away.world()));
    assert_eq!(
        live.world().resource::<Weather>().events,
        away.world().resource::<Weather>().events
    );
    assert_eq!(
        live.world().resource::<Shop>().stock,
        away.world().resource::<Shop>().stock
    );
    assert!(away
        .world()
        .resource::<Farm>()
        .away
        .starts_with("While you were away"));
}

#[test]
fn watering_once_preserves_progress_and_old_events_cannot_regrow_the_plant() {
    for delay in [100.0, 4900.0, 5100.0, 14900.0, 19000.0] {
        let mut game = new(7);
        game.tap("KeyE");
        game.run(delay);
        let e = game.world().resource::<Farm>().at([0, 0]).unwrap();
        let before = game.world().require::<Plant>(e).stage;
        let old_deadline = {
            let p = game.world().require::<Plant>(e);
            p.planted + p.grow_ms
        };
        let at = now_ms(game.world());
        game.tap("KeyQ");
        game.run(100.0);
        let p = game.world().require::<Plant>(e);
        assert!(p.watered);
        assert!(p.stage >= before && p.stage <= 4);
        let deadline = p.planted + p.grow_ms;
        let expected = at + (old_deadline - at) * 3 / 4;
        assert!(deadline.abs_diff(expected) <= 34, "{deadline} / {expected}");
        drop(p);
        assert_eq!(game.world().resource::<Farm>().water, 2);
        game.tap("KeyQ");
        game.run(100.0);
        assert_eq!(game.world().resource::<Farm>().water, 2, "no repeat dose");
        assert!(game
            .world()
            .published("care")
            .unwrap()
            .text()
            .contains("Watered"));
        let saved = game.save().unwrap();
        let mut restored = new(7);
        restored.restore(&saved).unwrap();
        // Cross both the accelerated and obsolete deadlines, then another minute.
        game.run(80_000.0);
        restored.run(80_000.0);
        assert_eq!(game.save().unwrap(), restored.save().unwrap());
        assert_eq!(game.world().require::<Plant>(e).stage, 4);
        let census = game.world().resource::<Census>();
        assert_eq!((census.plants, census.fruit, census.ripe), (1, 1, 1));
    }
}

#[test]
fn watering_fruit_resets_after_harvest_and_refilling_requires_the_barrel() {
    let mut game = new(7);
    {
        let mut farm = game.world_mut().resource_mut::<Farm>();
        farm.seeds[1] = 1;
        farm.held = Some(1);
    }
    game.tap("KeyE");
    game.run(100.0);
    game.tap("KeyQ");
    game.run(30_100.0);
    let plant = game.world().resource::<Farm>().at([0, 0]).unwrap();
    assert_eq!(game.world().require::<Plant>(plant).stage, 4);
    assert_eq!(game.world().resource::<Census>().ripe, 0);
    let old_ripe = garden_logic::garden::fruits_of(game.world(), plant)[0].2;
    game.tap("KeyQ");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 1);
    let fruit = garden_logic::garden::fruits_of(game.world(), plant);
    assert!(fruit
        .iter()
        .all(|&(e, ripe, at)| !ripe && at < old_ripe && game.world().require::<Fruit>(e).watered));
    game.tap("KeyQ");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 1);
    game.run(22_500.0);
    assert_eq!(
        game.world().resource::<Census>().ripe,
        4,
        "accelerated fruit ripens before its original deadline"
    );
    game.tap("KeyE");
    game.run(100.0);
    game.tap("KeyQ");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 0);
    game.tap("KeyQ");
    game.run(100.0);
    assert!(game.world().resource::<Farm>().last.contains("Can empty"));
    game.hold("KeyD", 1000.0);
    game.run(100.0);
    game.tap("KeyR");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 0, "no remote refill");
    for _ in 0..20 {
        if garden_logic::farm::at_barrel(game.world()) {
            break;
        }
        let hint = game.world().published("refill").unwrap().text().to_string();
        let key = [
            ("west", "KeyA"),
            ("east", "KeyD"),
            ("north", "KeyW"),
            ("south", "KeyS"),
        ]
        .iter()
        .find(|(word, _)| hint.contains(word))
        .unwrap()
        .1;
        game.hold(key, 200.0);
        game.run(100.0);
    }
    game.tap("KeyR");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 3);
    game.run(60_000.0);
    let census = game.world().resource::<Census>();
    assert_eq!((census.plants, census.fruit, census.ripe), (1, 4, 4));
}

#[test]
fn refill_availability_updates_at_the_range_boundary_within_one_plot_and_second() {
    let mut game = new(7);
    game.tap("KeyE");
    game.run(100.0);
    game.tap("KeyQ");
    game.run(100.0);
    game.world_mut()
        .require_mut::<Transform>("player")
        .position
        .x = 0.6;
    game.run(100.0);
    assert!(!game
        .world()
        .published("refill_ready")
        .unwrap()
        .as_bool()
        .unwrap());
    let plot = game.world().published("plot").unwrap().text().to_string();
    game.world_mut()
        .require_mut::<Transform>("player")
        .position
        .x = 0.4;
    game.run(100.0);
    assert_eq!(game.world().published("plot").unwrap().text(), plot);
    assert!(now_ms(game.world()) < 1000);
    assert!(game
        .world()
        .published("refill_ready")
        .unwrap()
        .as_bool()
        .unwrap());
}

#[test]
fn a_later_epoch_grows_the_garden_offline() {
    let epoch = 1.8e12;
    let mut game = Sim::<Garden>::new(Options {
        seed: 9,
        epoch,
        ..Options::default()
    })
    .unwrap();
    game.tap("KeyE");
    game.run(2_000.0);
    assert_eq!(game.world().resource::<Census>().ripe, 0);
    let saved = game.save().unwrap();
    // A new session an hour later restores that save.
    let mut later = Sim::<Garden>::new(Options {
        seed: 9,
        epoch: epoch + 3_600_000.0,
        ..Options::default()
    })
    .unwrap();
    later.restore_bound(&saved).unwrap();
    later.run(100.0);
    let clock = later.world().resource::<GardenClock>().offline_ms;
    assert!((3_597_000..=3_600_000).contains(&clock), "{clock}");
    assert_eq!(later.world().resource::<Census>().ripe, 1);
    assert!(
        later.world().resource::<Farm>().away.contains("(59:58)"),
        "{}",
        later.world().resource::<Farm>().away
    );
    // The same epoch again is the same session: no second catch-up.
    later.run(1000.0);
    assert_eq!(later.world().resource::<GardenClock>().offline_ms, clock);
}

#[test]
fn save_restore_a_big_garden() {
    let mut game = new(1);
    send(&mut game, "fill 2000");
    game.run(5.0 * 60_000.0);
    let saved = game.save().unwrap();
    let mut back = new(1);
    back.restore(&saved).unwrap();
    assert_eq!(back.world().hash(), game.world().hash());
    game.run(60_000.0);
    back.run(60_000.0);
    assert_eq!(back.world().hash(), game.world().hash());
    assert_eq!(back.save().unwrap(), game.save().unwrap());
}

#[test]
fn harvest_all_and_sell_all_at_scale() {
    let mut game = new(2);
    send(&mut game, "fill 1000");
    game.run(16.0 * 60_000.0);
    let ripe = game.world().resource::<Census>().ripe;
    assert!(ripe > 1000, "{ripe}");
    send(&mut game, "harvest all");
    let bag = game.world().resource::<Farm>().bag.len() as u32;
    assert!(bag >= ripe, "{bag} ≥ {ripe}");
    assert_eq!(
        game.world().published("bag_count").unwrap().as_number(),
        Some(bag as f64)
    );
    let before = sheckles(&game);
    send(&mut game, "sell all");
    assert!(sheckles(&game) > before + 10_000);
    let plants = game.world().query::<&Plant>().iter().count() as u32;
    assert_eq!(plants, game.world().resource::<Census>().plants);
    let fruit = game.world().query::<&Fruit>().iter().count() as u32;
    assert_eq!(fruit, game.world().resource::<Census>().fruit);
}

#[test]
fn expand_costs_and_grows() {
    let mut game = new(7);
    send(&mut game, "expand");
    assert_eq!(game.world().resource::<Farm>().size, 6);
    game.world_mut().resource_mut::<Farm>().sheckles = 1_000_000;
    send(&mut game, "expand");
    assert_eq!(game.world().resource::<Farm>().size, 8);
    assert_eq!(sheckles(&game), 1_000_000 - 6 * 6 * 6 * 25);
}

/// Two commands between two ticks are two messages: both apply.
#[test]
fn two_commands_in_one_tick_both_apply() {
    let mut game = new(7);
    game.post("buy carrot");
    game.post("buy carrot");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().seeds[0], 3);
}

#[test]
fn schedule_stays_bounded() {
    let mut game = new(4);
    send(&mut game, "fill 500");
    game.run(60.0 * 60_000.0);
    // Ripe fruit waits on the vine, so nothing more is due for it: the queue
    // holds unripe fruit, growing plants, the shop and the weather.
    let queued = game.world().resource::<Schedule>().heap.len() as u32;
    let census = game.world().resource::<Census>();
    assert!(
        queued <= census.fruit - census.ripe + census.plants + 2,
        "{queued}"
    );
    assert_eq!(CROPS.len(), 14);
}

#[test]
fn market_delivers_the_requested_fruit_once_and_preserves_its_full_value() {
    use garden_logic::garden::Item;
    let mut game = new(7);
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), 20);
    assert_eq!(game.world().resource::<Farm>().orders, 0);
    let carrot = Item {
        id: 0,
        kind: 0,
        weight: 0.5,
        muts: 0,
    };
    let extra = Item {
        id: 1,
        kind: 2,
        weight: 0.18,
        muts: 0,
    };
    let value = carrot.value();
    game.world_mut().resource_mut::<Farm>().bag = vec![extra, carrot];
    game.world_mut().resource_mut::<Shop>().stock[1] = 0;
    let saved = game.save().unwrap();
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), 20 + value + 30);
    assert_eq!(game.world().resource::<Farm>().orders, 1);
    assert_eq!(game.world().resource::<Shop>().stock[1], 1);
    assert_eq!(game.world().resource::<Farm>().bag.len(), 1);
    assert_eq!(game.world().resource::<Farm>().bag[0].kind, 2);
    assert!(game
        .world()
        .published("order")
        .unwrap()
        .text()
        .contains("4 Strawberry"));
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), 20 + value + 30);
    let mut restored = new(7);
    restored.restore_bound(&saved).unwrap();
    send(&mut restored, "deliver");
    send(&mut restored, "deliver");
    assert_eq!(restored.save().unwrap(), game.save().unwrap());
    // A malformed sell request must not silently mean sell everything.
    send(&mut game, "sell typo");
    assert_eq!(game.world().resource::<Farm>().bag.len(), 1);
    assert_eq!(sheckles(&game), 20 + value + 30);
}

#[test]
fn active_market_crop_is_available_after_every_restock() {
    let mut game = new(7);
    // Tomato has a 50% appearance chance; its active request survives many
    // rolls without gifting the player a seed or changing the seed price.
    game.world_mut().resource_mut::<Farm>().orders = 3;
    for _ in 0..12 {
        game.run(300_100.0);
        assert!(game.world().resource::<Shop>().stock[3] > 0);
    }
    assert_eq!(sheckles(&game), 20);
    assert_eq!(game.world().resource::<Farm>().seeds[3], 0);
}

#[test]
fn order_guidance_tracks_equipped_seed_and_movement_between_empty_tiles() {
    let mut game = new(7);
    {
        let mut farm = game.world_mut().resource_mut::<Farm>();
        farm.orders = 2;
        farm.seeds[0] = 2;
        farm.seeds[2] = 1;
    }
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(
        game.world().published("order_seed").unwrap().text(),
        "blueberry"
    );
    assert!(game
        .world()
        .published("order_hint")
        .unwrap()
        .text()
        .starts_with("Hold your Blueberry"));
    send(&mut game, "equip blueberry");
    assert!(game
        .world()
        .published("order_hint")
        .unwrap()
        .text()
        .contains("Move to an empty tile"));
    game.key_down("KeyD");
    game.run(500.0);
    game.key_up("KeyD");
    game.run(100.0);
    assert_eq!(
        game.world().published("plot").unwrap().text(),
        "Plot 2, 1 · Empty"
    );
    assert!(game
        .world()
        .published("order_hint")
        .unwrap()
        .text()
        .starts_with("Press E to plant Blueberry"));
    let saved = game.save().unwrap();
    let mut restored = new(7);
    restored.restore(&saved).unwrap();
    for sim in [&mut game, &mut restored] {
        // The next boundary is crossed after this second's timer publication.
        // The planting prompt stays identical; changing the tile must publish too.
        sim.key_down("KeyD");
        sim.run(400.0);
        sim.key_up("KeyD");
        sim.run(100.0);
        assert_eq!(
            sim.world().published("plot").unwrap().text(),
            "Plot 3, 1 · Empty"
        );
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(
            sim.world().published("plot").unwrap().text(),
            "Plot 3, 1 · Blueberry"
        );
        assert!(sim
            .world()
            .published("order_hint")
            .unwrap()
            .text()
            .starts_with("Wait here for Blueberry"));
    }
    assert!(
        game.save().unwrap() == restored.save().unwrap(),
        "guided planting must continue identically after restore"
    );
}

#[test]
fn a_lost_player_can_follow_the_public_prompt_after_restore() {
    let mut game = new(7);
    // The native Jev run ended near (1.89, -12), north of the last row.
    game.key_down("KeyD");
    game.run(500.0);
    game.key_up("KeyD");
    game.run(100.0);
    game.key_down("KeyW");
    game.run(4_000.0);
    game.key_up("KeyW");
    game.run(100.0);
    assert_eq!(
        game.world().require::<Transform>("player").position.z,
        -12.0
    );
    assert_eq!(
        game.world().published("plot").unwrap().text(),
        "Outside the garden"
    );
    assert!(game
        .world()
        .published("prompt")
        .unwrap()
        .text()
        .starts_with("Return to garden: south (S)"));
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().seeds[0], 1);
    assert_eq!(game.world().resource::<Census>().plants, 0);
    let saved = game.save().unwrap();
    let mut restored = new(7);
    restored.restore(&saved).unwrap();
    for sim in [&mut game, &mut restored] {
        sim.key_down("KeyS");
        sim.run(500.0);
        sim.key_up("KeyS");
        sim.run(100.0);
        assert_eq!(
            sim.world().published("plot").unwrap().text(),
            "Plot 2, 6 · Empty"
        );
        assert_eq!(
            sim.world().published("prompt").unwrap().text(),
            "E: plant Carrot (1 left)"
        );
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(sim.world().resource::<Census>().plants, 1);
    }
    assert!(game.save().unwrap() == restored.save().unwrap());
}

#[test]
fn return_guidance_reaches_the_garden_from_every_edge_and_corner() {
    use exact_game::Vec3;
    use garden_logic::farm;
    for size in [farm::START_SIZE, 16] {
        for (x, z) in [
            (-1., 0.),
            (1., 0.),
            (0., -1.),
            (0., 1.),
            (-1., -1.),
            (-1., 1.),
            (1., -1.),
            (1., 1.),
        ] {
            let mut game = new(7);
            farm::resize(game.world_mut(), size);
            let edge = size as f32 * 2.0;
            game.world_mut().require_mut::<Transform>("player").position =
                Vec3::new(x * edge, 0.9, z * edge);
            game.run(100.0);
            // Read only the same instructions a player sees; the next step
            // may turn a corner. No hidden position chooses the movement.
            for _ in 0..40 {
                let prompt = game.world().published("prompt").unwrap().text().to_owned();
                if prompt.starts_with("E: plant") {
                    break;
                }
                let key = [
                    ("north (W)", "KeyW"),
                    ("east (D)", "KeyD"),
                    ("south (S)", "KeyS"),
                    ("west (A)", "KeyA"),
                ]
                .into_iter()
                .find(|(direction, _)| prompt.contains(direction))
                .unwrap_or_else(|| panic!("no return direction: {prompt:?}"))
                .1;
                game.key_down(key);
                game.run(500.0);
                game.key_up(key);
                game.run(100.0);
            }
            let p = game.world().require::<Transform>("player").position;
            assert!(
                farm::tile_at(game.world(), p).is_some(),
                "lost at {p:?}, size {size}"
            );
            assert!(game.world().require::<Visible>("plot-north").0);
            assert_eq!(sheckles(&game), farm::START_SHECKLES);
            assert_eq!(game.world().resource::<Farm>().seeds[0], 1);
        }
    }
}

#[test]
fn an_occupied_north_row_guides_planting_after_restore() {
    let mut game = new(7);
    game.key_down("KeyD");
    game.run(500.0);
    game.key_up("KeyD");
    game.run(100.0);
    game.key_down("KeyW");
    game.run(4_000.0);
    game.key_up("KeyW");
    game.run(100.0);
    game.key_down("KeyS");
    game.run(500.0);
    game.key_up("KeyS");
    game.run(100.0);
    game.tap("KeyE");
    send(&mut game, "buy carrot");
    let guidance = game.world().published("planting");
    assert!(
        guidance
            .as_ref()
            .is_some_and(|p| p.text().starts_with("Empty plot")),
        "an occupied edge plot must show where to plant the held seed"
    );
    assert!(game
        .world()
        .published("prompt")
        .unwrap()
        .text()
        .starts_with("Carrot growing"));
    let saved = game.save().unwrap();
    let mut restored = new(7);
    restored.restore(&saved).unwrap();
    for sim in [&mut game, &mut restored] {
        follow_empty_plot(sim);
        assert_eq!(sim.world().published("planting").unwrap().text(), "");
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(sim.world().resource::<Census>().plants, 2);
        assert_eq!(sim.world().resource::<Farm>().seeds[0], 0);
        assert_eq!(sim.world().published("planting").unwrap().text(), "");
    }
    assert!(game.save().unwrap() == restored.save().unwrap());
}

fn follow_empty_plot(game: &mut Sim<Garden>) {
    for _ in 0..80 {
        if game
            .world()
            .published("prompt")
            .unwrap()
            .text()
            .starts_with("E: plant")
        {
            return;
        }
        let hint = game
            .world()
            .published("planting")
            .unwrap()
            .text()
            .to_owned();
        let key = [
            ("north (W)", "KeyW"),
            ("east (D)", "KeyD"),
            ("south (S)", "KeyS"),
            ("west (A)", "KeyA"),
        ]
        .into_iter()
        .find(|(direction, _)| hint.contains(direction))
        .unwrap_or_else(|| panic!("no empty-plot direction: {hint:?}"))
        .1;
        game.key_down(key);
        game.run(500.0);
        game.key_up(key);
        game.run(100.0);
    }
    panic!("visible empty-plot directions did not reach a planting tile");
}

#[test]
fn planting_guidance_updates_when_harvesting_or_expanding_a_full_garden() {
    let mut game = new(7);
    send(&mut game, "fill 36");
    assert_eq!(
        game.world().published("planting").unwrap().text(),
        "Garden full · expand to add empty plots"
    );
    game.run(20_100.0);
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(game.world().published("planting").unwrap().text(), "");
    // The only empty tile is now the harvested carrot at the opposite corner.
    game.world_mut().require_mut::<Transform>("player").position =
        exact_game::Vec3::new(10.0, 0.9, -10.0);
    game.run(1_000.0);
    follow_empty_plot(&mut game);
    game.tap("KeyE");
    send(&mut game, "buy carrot");
    assert_eq!(
        game.world().published("planting").unwrap().text(),
        "Garden full · expand to add empty plots"
    );
    garden_logic::farm::resize(game.world_mut(), 8);
    game.run(1_000.0);
    follow_empty_plot(&mut game);
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(game.world().resource::<Census>().plants, 37);
}

#[test]
fn plot_outline_tracks_growth_harvest_movement_and_restore() {
    let mut game = new(7);
    let color = |game: &Sim<Garden>| game.world().require::<Material>("plot-north").color;
    let empty = color(&game);
    assert!(game.world().require::<Visible>("plot-north").0);
    game.tap("KeyE");
    game.run(100.0);
    let growing = color(&game);
    assert_ne!(growing, empty);
    game.run(20_100.0);
    let ripe = color(&game);
    assert_ne!(ripe, growing);
    assert_ne!(ripe, empty);
    let saved = game.save().unwrap();
    let mut restored = new(7);
    restored.restore(&saved).unwrap();
    assert_eq!(color(&restored), ripe);
    for sim in [&mut game, &mut restored] {
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(color(sim), empty, "single harvest frees the tile");
        sim.key_down("KeyD");
        sim.run(500.0);
        sim.key_up("KeyD");
        sim.run(100.0);
        assert_eq!(
            sim.world().require::<Transform>("plot-north").position.x,
            2.0
        );
        assert_eq!(
            sim.world().require::<Transform>("plot-south").position.x,
            2.0
        );
        sim.key_down("KeyS");
        sim.run(500.0);
        sim.key_up("KeyS");
        sim.run(100.0);
        for edge in ["plot-north", "plot-south", "plot-east", "plot-west"] {
            assert!(!sim.world().require::<Visible>(edge).0, "outside: {edge}");
        }
        sim.key_down("KeyW");
        sim.run(500.0);
        sim.key_up("KeyW");
        sim.run(100.0);
        assert!(sim.world().require::<Visible>("plot-north").0);
    }
    assert!(game.save().unwrap() == restored.save().unwrap());
}

#[test]
fn stems_and_fruit_keep_the_same_offset_through_growth_and_regrowth() {
    for smooth in [false, true] {
        let mut game = Sim::<Garden>::new(Options {
            seed: 7,
            smooth,
            ..Options::default()
        })
        .unwrap();
        send(&mut game, "fill 14");
        let anchors: Vec<_> = game
            .world()
            .query::<&Plant>()
            .iter()
            .map(|(e, p)| {
                let pos = game.world().require::<Transform>(e).position;
                let tile = garden_logic::garden::tile_center(p.tile);
                assert!(
                    pos.x > tile.x + 0.6,
                    "stem must stand beside the tile centre"
                );
                (e, pos.x, pos.z)
            })
            .collect();
        for span in [10_000.0, 900_000.0] {
            game.run(span);
            for &(e, x, z) in &anchors {
                let pos = game.world().require::<Transform>(e).position;
                assert_eq!((pos.x, pos.z), (x, z), "growth moves only height and scale");
            }
        }
        for _ in 0..2 {
            assert!(game.world().resource::<Census>().ripe > 0);
            for (e, fruit) in game.world().query::<&Fruit>().iter() {
                let plant = game
                    .world()
                    .require::<Transform>(fruit.plant.unwrap())
                    .position;
                let pos = game.world().require::<Transform>(e).position;
                let dx = pos.x - plant.x;
                let dz = pos.z - plant.z;
                assert!(
                    dx * dx + dz * dz <= 0.43 * 0.43,
                    "fruit must share its stem's anchor"
                );
            }
            send(&mut game, "harvest all");
            game.run(900_000.0);
        }
    }
}

#[test]
fn market_bonus_stops_after_the_last_request() {
    use garden_logic::farm::ORDERS;
    use garden_logic::garden::Item;
    let mut game = new(7);
    for &(kind, count, bonus) in ORDERS {
        let before = sheckles(&game);
        let item = Item {
            id: 0,
            kind,
            weight: CROPS[kind as usize].weight,
            muts: 0,
        };
        let value = item.value();
        game.world_mut().resource_mut::<Farm>().bag = vec![item; count as usize];
        send(&mut game, "deliver");
        assert_eq!(sheckles(&game), before + value * count as u64 + bonus);
        assert!(game.world().resource::<Farm>().bag.is_empty());
    }
    let before = sheckles(&game);
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), before);
    assert_eq!(game.world().resource::<Farm>().orders, ORDERS.len() as u32);
    assert!(game
        .world()
        .published("order")
        .unwrap()
        .text()
        .contains("all 5 orders filled"));
}
