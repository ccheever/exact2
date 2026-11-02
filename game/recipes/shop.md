# Recipe: a shop on the number keys

Bind each item to a digit through `Actions`, keep what the player owns in a
resource, and publish one row per item for the HUD: its key, name, price and the
reason it cannot be bought, if any. A HUD button holds the same action, so a
click, a tap and the key all buy through one path.

```rust
use exact_game::*;
/// One shop row as the HUD shows it.
#[derive(Clone, Debug, Default, PartialEq, Data)]
struct Row { key: String, name: String, price: u32, action: String, reason: String }
#[derive(Default, Data)]
struct Hud { coins: u32, shop: Vec<Row> }
/// What the player owns: money and each item's level.
#[derive(Default, Resource)]
struct Shop { coins: u32, speed: u32, magnet: bool }
/// Key, name and action of each item, in shop order.
const ITEMS: [(&str, &str, &str); 2] = [("1", "Speed", "buy-speed"), ("2", "Magnet", "buy-magnet")];
/// The price of item `i` at the owned levels, or why it is locked.
fn price(shop: &Shop, i: usize) -> Result<u32, &'static str> {
    match i {
        0 if shop.speed == 3 => Err("Max level"),
        0 => Ok(4 * (shop.speed + 1)),
        _ if shop.magnet => Err("Owned"),
        _ if shop.speed == 0 => Err("Needs Speed"),
        _ => Ok(6),
    }
}
fn rows(shop: &Shop) -> Vec<Row> {
    ITEMS.iter().enumerate().map(|(i, &(key, name, action))| {
        let (price, reason) = match price(shop, i) {
            Ok(p) if p > shop.coins => (p, format!("Need {p}")),
            Ok(p) => (p, String::new()),
            Err(lock) => (0, lock.into()),
        };
        Row { key: key.into(), name: name.into(), price, action: action.into(), reason }
    }).collect()
}
struct Store;
impl Game for Store {
    const ID: &'static str = "recipe-shop";
    type Args = ();
    fn actions() -> Actions {
        Actions::new().button("buy-speed", &["Digit1"]).button("buy-magnet", &["Digit2"])
    }
    fn register(w: &mut World, _: &std::collections::BTreeMap<&str, Value>) {
        w.register_resource::<Shop>();
    }
    fn setup(w: &mut World, _: &()) { w.insert_resource(Shop { coins: 10, ..Shop::default() }); }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut shop = w.resource_mut::<Shop>();
        for (i, (_, _, action)) in ITEMS.iter().enumerate() {
            // `pressed`, not `held`: a tap can go down and up between two ticks.
            if !input.pressed(action) { continue; }
            let Ok(p) = price(&shop, i) else { continue };
            if p > shop.coins { continue; }
            shop.coins -= p;
            match i { 0 => shop.speed += 1, _ => shop.magnet = true }
        }
        w.publish_record(&Hud { coins: shop.coins, shop: rows(&shop) });
    }
}
let mut sim = Sim::<Store>::new(()).unwrap();
let shown = |sim: &Sim<Store>| rows(&sim.world().resource::<Shop>());
sim.run(20.);
assert_eq!(shown(&sim)[1].reason, "Needs Speed");
sim.tap("Digit2");                 // locked: nothing happens
sim.run(20.);
sim.tap("Digit1");                 // down and up inside one tick: bought once
sim.run(20.);
assert_eq!((sim.world().resource::<Shop>().speed, sim.world().resource::<Shop>().coins), (1, 6));
assert_eq!(shown(&sim)[0].reason, "Need 8");
assert_eq!((shown(&sim)[1].price, shown(&sim)[1].reason.as_str()), (6, ""));
sim.tap("Digit2");
sim.run(20.);
assert_eq!(shown(&sim)[1].reason, "Owned");
assert_eq!(sim.world().published("coins").unwrap().as_number(), Some(0.));
```

The HUD renders the rows (`app.contract`):

```text
shape ShopRow
  key: string
  name: string
  price: number
  action: string
  reason: string

shape Hud
  coins: number
  shop: ShopRow[]

…
          column testId="shop" position="absolute" top=76 right=24 gap=8
            for r in hud.shop key=r.key
              button action=r.action disabled=(r.reason != "") testId=`buy-${r.key}` aria-label=(r.reason == "" ? `${r.name}, ${r.price} ${r.price == 1 ? "coin" : "coins"}, key ${r.key}` : `${r.name}, ${r.reason}`) padding=12 border-radius=12 background-color="#202731cc"
                text (r.reason == "" ? `${r.key} ${r.name} · ${r.price}` : `${r.key} ${r.name} · ${r.reason}`) font-variant-numeric="tabular-nums"
```

- The button's `action=` holds the world's action while it is pressed; a
  disabled button presses nothing. The digit reaches the world through
  `Actions`, so the button carries no `aria-keyshortcuts`: a shortcut on a key the
  game also binds takes the key before the world hears it, and the bake warns
  (`analyze-shortcut-world-key`, [game pitfalls](../../docs/game-pitfalls.md)).
- Buy on `input.pressed`: a `tap world Digit1` or a quick key lands inside one
  tick ([same-tick press](same-tick-press.md)).
- Levels are state and live in the resource; prices and locks are rules, computed
  from it each tick, so a restored save shows the same rows.
- The [starter](../new/logic/src/lib.rs) has a two-item shop, and its
  `logic/tests/sim.rs` a bot that plays five minutes to check the prices against
  the income. This fragment is a doc-test.
