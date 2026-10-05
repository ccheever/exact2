use exact_game::{DrawnMesh, Material, Sim, Transform, Visible};
use garden_logic::crops::Balance;
use garden_logic::farm::Farm;
use garden_logic::garden::{now_ms, Census, Fruit, GardenClock, Plant, Schedule, Weather};
use garden_logic::shop::Shop;
use garden_logic::{census_of, Garden, Options};

/// The balance, as the game reads it.
static BAL: std::sync::LazyLock<Balance> = std::sync::LazyLock::new(|| {
    exact_game::json::from_str(include_str!("../../assets/garden.level.json")).unwrap()
});

/// A garden whose declared data (the balance and the looks) has arrived from
/// the game's `assets/`, as a host delivers it before setup.
fn garden(options: Options) -> Sim<Garden> {
    let mut sim = Sim::<Garden>::new(options).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets");
    for name in ["garden.level.json", "looks.level.json"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        sim.asset(name, Some(&bytes)).unwrap();
    }
    sim
}

/// What the HUD shows now: the status record, as values.
fn hud(game: &Sim<Garden>) -> garden_logic::hud::Status {
    garden_logic::hud::status(game.world())
}

/// The status the world last published, if it published since the last look.
fn published(game: &mut Sim<Garden>) -> Option<garden_logic::hud::Status> {
    game.take_published()
        .map(|json| exact_game::json::from_str(&json).unwrap())
}

/// The movement key for a published way across the garden.
fn key(dir: &str) -> &'static str {
    match dir {
        "north" => "KeyW",
        "south" => "KeyS",
        "east" => "KeyD",
        "west" => "KeyA",
        _ => panic!("no direction: {dir:?}"),
    }
}

fn plot(x: u32, z: u32, crop: &str) -> garden_logic::hud::Plot {
    garden_logic::hud::Plot {
        inside: true,
        x,
        z,
        crop: crop.into(),
    }
}

fn new(seed: u64) -> Sim<Garden> {
    garden(Options {
        seed,
        ..Options::default()
    })
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
    let prompt = hud(&game).prompt;
    assert!(
        prompt.what == "growing" && prompt.crop == "Carrot",
        "{prompt:?}"
    );
    // Not ripe yet: E harvests nothing.
    game.tap("KeyE");
    game.run(100.0);
    assert!(game.world().resource::<Farm>().bag.is_empty());
    game.run(20_000.0);
    assert_eq!(game.world().resource::<Census>().ripe, 1);
    let prompt = hud(&game).prompt;
    assert_eq!(
        (prompt.what.as_str(), prompt.count, prompt.crop.as_str()),
        ("harvest", 1, "Carrot")
    );
    game.tap("KeyE");
    game.run(100.0);
    let bag = game.world().resource::<Farm>().bag.clone();
    assert_eq!(bag.len(), 1);
    // A carrot is single-harvest: the plant is gone, the tile free.
    assert_eq!(game.world().resource::<Census>().plants, 0);
    assert!(game.world().resource::<Farm>().at([0, 0]).is_none());
    let value = bag[0].value(&BAL);
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
    let last = game.world().resource::<Farm>().last.clone();
    assert!(last.what == "costs" && last.coins == 50, "{last:?}");
    game.world_mut().resource_mut::<Farm>().sheckles = 1000;
    let berry = BAL.kind_of("strawberry").unwrap();
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
    let restock = |game: &Sim<Garden>| game.world().published("restock_s").unwrap().as_number();
    assert_eq!(restock(&game), Some(300.0));
    game.run(60_100.0);
    assert_eq!(restock(&game), Some(240.0));
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
    live.world_mut().resource_mut::<Farm>().plant_food = 1;
    live.tap("KeyQ");
    live.tap("KeyF");
    live.run(100.0);
    let mut away = new(5);
    send(&mut away, "fill 300");
    away.world_mut().resource_mut::<Farm>().plant_food = 1;
    away.tap("KeyQ");
    away.tap("KeyF");
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
    assert!(away.world().resource::<Farm>().away.events > 0);
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
        assert_eq!(game.world().published("care").unwrap().text(), "watered");
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
    assert_eq!(game.world().resource::<Farm>().last.what, "can_empty");
    game.hold("KeyD", 1000.0);
    game.run(100.0);
    game.tap("KeyR");
    game.run(100.0);
    assert_eq!(game.world().resource::<Farm>().water, 0, "no remote refill");
    for _ in 0..20 {
        if garden_logic::farm::at_barrel(game.world()) {
            break;
        }
        let way = hud(&game).barrel.way;
        game.hold(key(&way.dir), 200.0);
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
    let plot = hud(&game).plot;
    game.world_mut()
        .require_mut::<Transform>("player")
        .position
        .x = 0.4;
    game.run(100.0);
    assert_eq!(hud(&game).plot, plot);
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
    let mut game = garden(Options {
        seed: 9,
        epoch,
        ..Options::default()
    });
    game.tap("KeyE");
    game.run(2_000.0);
    assert_eq!(game.world().resource::<Census>().ripe, 0);
    let saved = game.save().unwrap();
    // A new session an hour later restores that save.
    let mut later = garden(Options {
        seed: 9,
        epoch: epoch + 3_600_000.0,
        ..Options::default()
    });
    later.restore_bound(&saved).unwrap();
    later.run(100.0);
    let clock = later.world().resource::<GardenClock>().offline_ms;
    assert!((3_597_000..=3_600_000).contains(&clock), "{clock}");
    assert_eq!(later.world().resource::<Census>().ripe, 1);
    let away = later.world().resource::<Farm>().away.clone();
    assert_eq!(away.s, 3598, "{away:?}");
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
    assert_eq!(BAL.crops.len(), 14);
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
        fed: true,
    };
    let extra = Item {
        id: 1,
        kind: 2,
        weight: 0.18,
        muts: 0,
        fed: false,
    };
    let value = carrot.value(&BAL);
    game.world_mut().resource_mut::<Farm>().bag = vec![extra, carrot];
    game.world_mut().resource_mut::<Shop>().stock[1] = 0;
    let saved = game.save().unwrap();
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), 20 + value + 30);
    assert_eq!(game.world().resource::<Farm>().orders, 1);
    assert_eq!(game.world().resource::<Shop>().stock[1], 1);
    assert_eq!(game.world().resource::<Farm>().bag.len(), 1);
    assert_eq!(game.world().resource::<Farm>().bag[0].kind, 2);
    let order = hud(&game).order;
    assert_eq!((order.count, order.crop.as_str()), (4, "Strawberry"));
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
    let hint = hud(&game).order_hint;
    assert_eq!(
        (hint.what.as_str(), hint.crop.as_str()),
        ("hold_seed", "Blueberry")
    );
    send(&mut game, "equip blueberry");
    assert_eq!(hud(&game).order_hint.what, "move");
    game.key_down("KeyD");
    game.run(500.0);
    game.key_up("KeyD");
    game.run(100.0);
    assert_eq!(hud(&game).plot, plot(1, 0, ""));
    assert_eq!(hud(&game).order_hint.what, "plant_here");
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
        assert_eq!(published(sim).map(|s| s.plot), Some(plot(2, 0, "")));
        sim.tap("KeyE");
        sim.run(100.0);
        let shown = published(sim).unwrap();
        assert_eq!(shown.plot, plot(2, 0, "Blueberry"));
        assert_eq!(shown.order_hint.what, "wait_here");
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
    assert!(!hud(&game).plot.inside);
    let prompt = hud(&game).prompt;
    assert_eq!(
        (prompt.what.as_str(), prompt.way.dir.as_str()),
        ("return", "south")
    );
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
        assert_eq!(hud(sim).plot, plot(1, 5, ""));
        let prompt = hud(sim).prompt;
        assert_eq!(
            (prompt.what.as_str(), prompt.crop.as_str(), prompt.count),
            ("plant", "Carrot", 1)
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
    for size in [BAL.farm.start_size, 16] {
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
                let prompt = hud(&game).prompt;
                if prompt.what == "plant" {
                    break;
                }
                let key = key(&prompt.way.dir);
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
            assert_eq!(sheckles(&game), BAL.farm.start_sheckles);
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
    assert_eq!(
        hud(&game).planting.what,
        "empty",
        "an occupied edge plot must show where to plant the held seed"
    );
    assert_eq!(hud(&game).prompt.what, "growing");
    let saved = game.save().unwrap();
    let mut restored = new(7);
    restored.restore(&saved).unwrap();
    for sim in [&mut game, &mut restored] {
        follow_empty_plot(sim);
        assert_eq!(hud(sim).planting.what, "");
        sim.tap("KeyE");
        sim.run(100.0);
        assert_eq!(sim.world().resource::<Census>().plants, 2);
        assert_eq!(sim.world().resource::<Farm>().seeds[0], 0);
        assert_eq!(hud(sim).planting.what, "");
    }
    assert!(game.save().unwrap() == restored.save().unwrap());
}

fn follow_empty_plot(game: &mut Sim<Garden>) {
    for _ in 0..80 {
        if hud(game).prompt.what == "plant" {
            return;
        }
        let key = key(&hud(game).planting.way.dir);
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
    assert_eq!(hud(&game).planting.what, "full");
    game.run(20_100.0);
    game.tap("KeyE");
    game.run(100.0);
    assert_eq!(hud(&game).planting.what, "");
    // The only empty tile is now the harvested carrot at the opposite corner.
    game.world_mut().require_mut::<Transform>("player").position =
        exact_game::Vec3::new(10.0, 0.9, -10.0);
    game.run(1_000.0);
    follow_empty_plot(&mut game);
    game.tap("KeyE");
    send(&mut game, "buy carrot");
    assert_eq!(hud(&game).planting.what, "full");
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
        let mut game = garden(Options {
            seed: 7,
            smooth,
            ..Options::default()
        });
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
    use garden_logic::garden::Item;
    let mut game = new(7);
    for (kind, count, bonus) in (0..BAL.orders.len() as u32).filter_map(|i| BAL.order(i)) {
        let before = sheckles(&game);
        let item = Item {
            id: 0,
            kind,
            weight: BAL.crops[kind as usize].weight,
            muts: 0,
            fed: false,
        };
        let value = item.value(&BAL);
        game.world_mut().resource_mut::<Farm>().bag = vec![item; count as usize];
        send(&mut game, "deliver");
        assert_eq!(sheckles(&game), before + value * count as u64 + bonus);
        assert!(game.world().resource::<Farm>().bag.is_empty());
    }
    let before = sheckles(&game);
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), before);
    assert_eq!(
        game.world().resource::<Farm>().orders,
        BAL.orders.len() as u32
    );
    let order = hud(&game).order;
    assert!(order.done && order.total == 5, "{order:?}");
}

#[test]
fn compost_spends_only_the_chosen_fruit_and_refuses_stale_ids_or_a_full_pouch() {
    use garden_logic::garden::Item;
    let mut game = new(7);
    game.world_mut().resource_mut::<Farm>().bag = (0..4)
        .map(|id| Item {
            id,
            kind: 0,
            weight: 0.25,
            ..Item::default()
        })
        .collect();
    send(&mut game, "compost 1");
    assert_eq!(game.world().resource::<Farm>().plant_food, 1);
    assert_eq!(
        game.world()
            .resource::<Farm>()
            .bag
            .iter()
            .map(|i| i.id)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    for command in ["compost 1", "compost nope", "compost 99"] {
        send(&mut game, command);
        assert_eq!(game.world().resource::<Farm>().plant_food, 1);
        assert_eq!(game.world().resource::<Farm>().bag.len(), 3);
    }
    send(&mut game, "compost 0");
    send(&mut game, "compost 2");
    send(&mut game, "compost 3");
    assert_eq!(game.world().resource::<Farm>().plant_food, 3);
    assert_eq!(game.world().resource::<Farm>().bag[0].id, 3);
    assert_eq!(sheckles(&game), 20, "composting pays no sale proceeds");
    game.tap("KeyF");
    game.run(100.0);
    assert_eq!(
        game.world().resource::<Farm>().plant_food,
        3,
        "an empty tile spends nothing"
    );
    assert_eq!(
        game.world().published("bag_count").unwrap().as_number(),
        Some(1.0)
    );
    assert_eq!(
        game.world().published("plant_food").unwrap().as_number(),
        Some(3.0)
    );
    let bytes = game.save().unwrap();
    let mut back = new(7);
    back.restore(&bytes).unwrap();
    assert_eq!(back.world().resource::<Farm>().plant_food, 3);
    assert_eq!(back.world().resource::<Farm>().bag[0].id, 3);
}

#[test]
fn feeding_improves_one_harvest_without_rerolling_it_and_survives_restore() {
    for (kind, late) in [(0, false), (1, false), (1, true), (2, false), (2, true)] {
        let crop = &BAL.crops[kind as usize];
        let mut fed = new(7);
        let mut plain = new(7);
        for game in [&mut fed, &mut plain] {
            {
                let mut farm = game.world_mut().resource_mut::<Farm>();
                farm.held = Some(kind);
                farm.seeds[kind as usize] = 1;
                farm.plant_food = 3;
            }
            game.tap("KeyE");
            game.run(100.0);
            game.run(if late {
                crop.grow_s as f64 * 1000.0
            } else {
                3000.0
            });
        }
        fed.tap("KeyF");
        fed.run(100.0);
        plain.run(100.0);
        assert_eq!(fed.world().resource::<Farm>().plant_food, 2);
        let plant = fed.world().resource::<Farm>().at([0, 0]).unwrap();
        assert!(garden_logic::garden::is_fed(fed.world(), plant));
        assert!(!garden_logic::garden::needs_feed(fed.world(), plant));
        fed.tap("KeyF");
        fed.run(100.0);
        plain.run(100.0);
        assert_eq!(
            fed.world().resource::<Farm>().plant_food,
            2,
            "repeat feeding spends nothing"
        );
        let bytes = fed.save().unwrap();
        let mut back = new(7);
        back.restore(&bytes).unwrap();
        let span = (crop.grow_s + crop.fruit_s) as f64 * 1000.0 + 1000.0;
        for game in [&mut fed, &mut plain, &mut back] {
            game.run(span);
        }
        let fruit = |game: &Sim<Garden>| {
            game.world()
                .query::<&Fruit>()
                .iter()
                .map(|(_, f)| (f.weight, f.muts, f.fed, f.ripe))
                .collect::<Vec<_>>()
        };
        let boosted = fruit(&fed);
        let normal = fruit(&plain);
        assert_eq!(boosted.len(), crop.slots as usize);
        for (f, p) in boosted.iter().zip(&normal) {
            assert_eq!(f.0, p.0 * 1.25, "exactly one 25% weight boost");
            assert_eq!(f.1, p.1, "mutations do not reroll");
            assert!(f.2 && f.3 && !p.2 && p.3);
        }
        assert_eq!(fruit(&back), boosted);
        assert!(fed.save().unwrap() == back.save().unwrap());
        for game in [&mut fed, &mut back] {
            game.tap("KeyF");
            game.run(100.0);
        }
        plain.run(100.0);
        assert_eq!(
            fed.world().resource::<Farm>().plant_food,
            2,
            "ripe fruit cannot consume food"
        );
        for game in [&mut fed, &mut plain, &mut back] {
            game.tap("KeyE");
            game.run(100.0);
        }
        assert!(fed.world().resource::<Farm>().bag.iter().all(|i| i.fed));
        assert!(fed.save().unwrap() == back.save().unwrap());
        if crop.regrows() {
            assert!(
                garden_logic::garden::needs_feed(fed.world(), plant),
                "new growth can be fed again"
            );
            for game in [&mut fed, &mut plain, &mut back] {
                game.run(crop.fruit_s as f64 * 1000.0 + 1000.0);
            }
            assert_eq!(
                fruit(&fed),
                fruit(&plain),
                "regrowth has no free second boost"
            );
            assert!(fed.save().unwrap() == back.save().unwrap());
        }
    }
}

/// A game in a look, bound the way the canvas binds its arguments.
fn in_look(seed: u64, art: &str) -> Sim<Garden> {
    garden(Options {
        seed,
        art: art.into(),
        ..Options::default()
    })
}

/// Switch a running game's look, as the Garden panel's Look row does.
fn switch(game: &mut Sim<Garden>, seed: u64, art: &str) {
    use exact_game::Args;
    let options = Options {
        seed,
        art: art.into(),
        ..Options::default()
    };
    game.bind(&options.values(), None).unwrap();
}

/// The looks are presentation only: the same inputs grow the same garden to
/// the same hash in every look, a save restores in any look, and switching
/// the look live keeps the garden and plays on as if it had never changed.
#[test]
fn every_look_plays_the_same_garden() {
    let play = |art: &str| {
        let mut game = in_look(3, art);
        send(&mut game, "fill 100");
        send(&mut game, "expand");
        game.run(3.0 * 60_000.0);
        game
    };
    let classic = play("");
    let hash = classic.world().hash();
    let saved = classic.save().unwrap();
    for art in ["golden", "storybook", "pass"] {
        let game = play(art);
        assert_eq!(census_of(game.world()), census_of(classic.world()), "{art}");
        assert_eq!(game.world().hash(), hash, "{art}: the look moved the world");
        // A classic save restores into this look and draws in it.
        let mut back = in_look(3, art);
        back.restore(&saved).unwrap();
        assert_eq!(back.world().hash(), hash, "{art}");
    }
    // Live: grow in the art pass, switch through every look; the garden stays.
    let mut live = in_look(3, "pass");
    send(&mut live, "fill 100");
    send(&mut live, "expand");
    live.run(60_000.0);
    for art in ["", "golden", "storybook", "pass", ""] {
        let before = (census_of(live.world()), live.world().hash());
        switch(&mut live, 3, art);
        assert_eq!(
            (census_of(live.world()), live.world().hash()),
            before,
            "switching to {art:?} rebuilt the garden"
        );
        let drawn = live.world().query::<&DrawnMesh>().iter().count();
        assert!(drawn > 0, "{art:?} draws its props from present");
    }
    live.run(2.0 * 60_000.0);
    assert_eq!(live.world().hash(), hash, "the switches changed nothing");
    // A look's models are made when it is drawn, outside the save: a garden
    // that showed every look saves as one that showed only classic.
    switch(&mut live, 3, "");
    assert!(
        live.save().unwrap() == saved,
        "the looks shown are in the save"
    );
    assert!(Sim::<Garden>::new(Options {
        art: "neon".into(),
        ..Options::default()
    })
    .is_err());
}

/// The art pass draws each plant's stage model standing on its tile and each
/// fruit on its branch, beside its plant, with its mutation's look; none of it
/// is simulation, and a restored garden draws the same.
#[test]
fn the_art_pass_hangs_fruit_on_its_plants_and_presents_mutations() {
    use exact_game::{MaterialOverrides, Mesh};
    let mut game = in_look(5, "pass");
    send(&mut game, "fill 28");
    game.run(900_000.0);
    let w = game.world();
    let drawn = |e| w.drawn(e).unwrap().pose.translation;
    let mut plants = 0;
    for (e, p) in w.query::<&Plant>().iter() {
        plants += 1;
        assert!(drawn(e).y.abs() < 1e-4, "pass plants stand on the ground");
        let swap = w.require::<DrawnMesh>(e);
        assert_eq!(
            swap.mesh,
            Mesh::asset(format!(
                "plant-{}-{}.model",
                BAL.crops[p.kind as usize].id, p.stage
            ))
        );
        assert!(swap.lod.is_some());
        // The simulation keeps the classic model.
        assert_eq!(
            *w.require::<Mesh>(e),
            Mesh::asset(format!("plant-{}.model", p.kind))
        );
    }
    assert!(plants > 0);
    let mut mutated = 0;
    for (e, f) in w.query::<&Fruit>().iter() {
        let plant = w.require::<Plant>(f.plant.unwrap());
        let at = drawn(e);
        let center = garden_logic::garden::tile_center(plant.tile);
        assert!(
            (at.x - center.x).abs() < 1.6 && (at.z - center.z).abs() < 1.6,
            "{} slot {} hangs off its tile: {at:?} from {center:?}",
            BAL.crops[f.kind as usize].id,
            f.slot
        );
        let Mesh::Asset(name) = &w.require::<DrawnMesh>(e).mesh else {
            panic!("a pass fruit is a baked model")
        };
        assert_eq!(name.ends_with("-unripe.model"), !f.ripe, "{name}");
        assert_eq!(
            w.get::<MaterialOverrides>(e).is_some(),
            f.ripe && f.muts != 0,
            "{name} {}",
            f.muts
        );
        mutated += (f.ripe && f.muts != 0) as u32;
    }
    assert!(mutated > 0, "the weather mutated some fruit");
    // A restored garden presents the same looks, rebuilt from the save.
    let saved = game.save().unwrap();
    let mut back = in_look(5, "pass");
    back.restore(&saved).unwrap();
    let looks = |game: &Sim<Garden>| -> Vec<String> {
        let w = game.world();
        w.query::<&Fruit>()
            .iter()
            .map(|(e, _)| {
                format!(
                    "{:?} {:?} {:?}",
                    w.get::<DrawnMesh>(e).map(|m| m.clone()),
                    w.get::<MaterialOverrides>(e).map(|m| m.clone()),
                    w.drawn(e).map(|d| d.pose)
                )
            })
            .collect()
    };
    assert_eq!(looks(&back), looks(&game));
}

/// The art pass's sky is presentation: a day and night, and each weather's
/// fog, move the drawn sun, moon and sky but never the simulation's.
#[test]
fn the_art_pass_draws_day_and_night_without_simulating_them() {
    use exact_game::{DirectionalLight, DrawnEnvironment, DrawnLight};
    let mut game = in_look(5, "pass");
    let lux = |game: &Sim<Garden>, name: &str| match *game.world().require::<DrawnLight>(name) {
        DrawnLight::Directional(l) => l.illuminance,
        _ => panic!("{name} is a directional light"),
    };
    let sun = *game.world().require::<DirectionalLight>("sun");
    let noon = lux(&game, "sun");
    let sky = game
        .world()
        .require::<DrawnEnvironment>("camera")
        .environment;
    // Half a garden day on: night.
    game.run(300_000.0);
    assert!(lux(&game, "sun") < noon * 0.1, "the sun sets");
    assert!(lux(&game, "moon") > 0.0, "the moon rises");
    let night = game
        .world()
        .require::<DrawnEnvironment>("camera")
        .environment;
    assert_ne!(night.zenith, sky.zenith);
    assert_eq!(
        *game.world().require::<DirectionalLight>("sun"),
        sun,
        "the simulated sun never moves"
    );
    assert!(game.world().get::<DirectionalLight>("moon").is_none());
}

/// Each look draws and downloads only its own baked models: classic none, a
/// styled look its `golden-`/`storybook-` set, the art pass the rest; a
/// switch to a look fetches what it shows first.
#[test]
fn each_look_fetches_only_its_own_models() {
    use exact_game::{Game, Mesh};
    let own = |art: &str, name: &str| match art {
        "" => false,
        "pass" => !name.starts_with("golden-") && !name.starts_with("storybook-"),
        art => name.starts_with(&format!("{art}-")),
    };
    let mut game = in_look(1, "");
    send(&mut game, "fill 30");
    game.run(900_000.0);
    for art in ["", "golden", "storybook", "pass"] {
        switch(&mut game, 1, art);
        let fetched = game.take_assets();
        assert!(
            fetched.iter().all(|n| own(art, n)),
            "look {art:?} fetches another look's models: {fetched:?}"
        );
        if !art.is_empty() {
            assert!(
                !fetched.is_empty() && fetched.len() < Garden::STREAMED.len(),
                "what {art:?} shows, alone first: {fetched:?}"
            );
        }
        game.run(100.0);
        let w = game.world();
        for (_, d) in w.query::<&DrawnMesh>().iter() {
            if let Mesh::Asset(name) = &d.mesh {
                if Garden::STREAMED.contains(&name.as_str()) {
                    assert!(own(art, name), "look {art:?} draws {name}");
                }
            }
        }
        game.take_assets();
    }
}

/// The art pass under the paranoid modes: every sampled tick rebuilds the
/// world through restore and presents again; play, hash and looks agree.
#[test]
fn the_art_pass_survives_paranoid_restores() {
    let run = |mode| {
        let mut game = in_look(9, "pass").paranoid(mode);
        send(&mut game, "fill 20");
        game.run(240_000.0);
        let w = game.world();
        let looks: Vec<String> = w
            .query::<&DrawnMesh>()
            .iter()
            .map(|(e, d)| format!("{e:?} {d:?} {:?}", w.drawn(e).map(|d| d.pose)))
            .collect();
        (census_of(w), w.hash(), looks)
    };
    let plain = run(exact_game::Paranoid::Off);
    assert_eq!(run(exact_game::Paranoid::Save), plain);
    assert_eq!(run(exact_game::Paranoid::FreshGame), plain);
}
