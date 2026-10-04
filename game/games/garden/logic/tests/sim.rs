use exact_game::Sim;
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
    let mut away = new(5);
    send(&mut away, "fill 300");
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
    let saved = game.save().unwrap();
    send(&mut game, "deliver");
    assert_eq!(sheckles(&game), 20 + value + 30);
    assert_eq!(game.world().resource::<Farm>().orders, 1);
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
