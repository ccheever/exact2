//! What the HUD reads. Three records, each published only when it changes:
//! the status (at most once a garden second, for its timers), the shop and
//! seeds, and the backpack. They are values (counts, names, ids, seconds,
//! what happened), never sentences: `app.contract` words them.

use crate::crops::{balance, Balance};
use crate::farm::{self, Farm};
use crate::garden::{now_ms, Census, Item, Plant, Schedule, Sky, Weather};
use crate::shop::Shop;
use exact_game::*;

/// Garden milliseconds as the whole seconds a countdown shows (rounded up).
pub fn secs(ms: u64) -> u64 {
    ms.div_ceil(1000)
}

/// A way across the garden's grid: "north", "south", "east" or "west" (the
/// larger axis, so its WASD key), and about how many metres.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Bearing {
    pub dir: String,
    pub metres: u32,
}

/// A fruit as the HUD names it: its crop, its mutations' names, and whether
/// it was fed (`app.contract`'s `fruitLabel`).
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Described {
    pub crop: String,
    pub muts: Vec<String>,
    pub fed: bool,
}
impl Described {
    pub fn of(b: &Balance, item: &Item) -> Self {
        Described {
            crop: b.crop(item.kind).name.clone(),
            muts: b
                .mutation_names(item.muts)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            fed: item.fed,
        }
    }
}

/// What an action did, or why it did nothing: `what` names it ("planted",
/// "costs", "can_empty", …) and the rest are its values. Saved with the farm,
/// so the line survives a restore; `app.contract`'s `noteText` words it.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Note {
    pub what: String,
    pub crop: String,
    pub count: u64,
    pub coins: u64,
    pub bonus: u64,
    pub fruit: Described,
    pub way: Bearing,
    /// A word the player typed (an unknown seed or command).
    pub word: String,
}
impl Note {
    pub fn new(what: &str) -> Self {
        Note {
            what: what.into(),
            ..Note::default()
        }
    }
    pub fn crop(mut self, name: &str) -> Self {
        self.crop = name.into();
        self
    }
    pub fn count(mut self, n: u64) -> Self {
        self.count = n;
        self
    }
    pub fn coins(mut self, n: u64) -> Self {
        self.coins = n;
        self
    }
    pub fn bonus(mut self, n: u64) -> Self {
        self.bonus = n;
        self
    }
    pub fn fruit(mut self, fruit: Described) -> Self {
        self.fruit = fruit;
        self
    }
    pub fn way(mut self, way: Bearing) -> Self {
        self.way = way;
        self
    }
    pub fn word(mut self, word: &str) -> Self {
        self.word = word.into();
        self
    }
}

/// What E does on this tile: "plant" (the held seed, `count` left), "buy"
/// (no seed in hand), "growing" or "fruiting" (`s` seconds to go), "harvest"
/// (`count` ripe); or "return" with the way back to the garden.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Prompt {
    pub what: String,
    pub crop: String,
    pub count: u32,
    pub s: u64,
    pub way: Bearing,
}

/// The player's plot (zero-based) and what grows there; `inside` is false
/// off the garden.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Plot {
    pub inside: bool,
    pub x: u32,
    pub z: u32,
    pub crop: String,
}

/// Where a held seed could go when this plot is taken: "empty" (the nearest
/// empty plot, `x`, `z`, and the way), "full" (expand for more), "maxed" (no
/// room at all); "" when there is nothing to say.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Planting {
    pub what: String,
    pub x: u32,
    pub z: u32,
    pub way: Bearing,
}

/// The market's current request: number `n` of `total` (zero-based), its
/// crop, count and bonus, and how many the backpack carries; `done` once
/// every request is filled.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Order {
    pub n: u32,
    pub total: u32,
    pub done: bool,
    pub crop: String,
    pub count: u32,
    pub bonus: u64,
    pub carried: u32,
}

/// The market's next step: "deliver", "harvest_here", "wait_here",
/// "hold_seed", "walk_on", "move" (this plot grows `other`), "plant_here",
/// "sold_out" (restock in `s`), "costs" (`price`), "buy"; "explore" once done.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Hint {
    pub what: String,
    pub crop: String,
    pub other: String,
    pub s: u64,
    pub price: u64,
}

/// How far the garden grew while away, and what ripened.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Away {
    pub s: u64,
    pub events: u64,
    pub ripened: u32,
    pub ripe: u32,
}

/// The blue barrel: whether the player is at it, and the way there.
#[derive(Clone, Default, Debug, PartialEq, Data)]
pub struct Barrel {
    pub at: bool,
    pub way: Bearing,
}

#[derive(Default, Data)]
pub struct Status {
    pub sheckles: u64,
    /// The held seed's id ("" for none), name and count.
    pub held: String,
    pub held_name: String,
    pub held_count: u32,
    pub prompt: Prompt,
    pub plot: Plot,
    pub planting: Planting,
    /// "clear", "rain", "snow" or "storm", and seconds until it changes.
    pub weather: String,
    pub weather_s: u64,
    pub restock_s: u64,
    pub plants: u32,
    pub fruit: u32,
    pub ripe: u32,
    pub mutated: u32,
    pub size: u32,
    pub expand_cost: u64,
    pub last: Note,
    pub away: Away,
    pub garden_s: u64,
    pub events: f64,
    pub queued: u32,
    pub order: Order,
    pub order_hint: Hint,
    pub order_seed: String,
    pub order_ready: bool,
    pub orders: u32,
    pub water: u32,
    pub water_max: u32,
    pub water_ready: bool,
    /// "empty" (the can), "water", "watered", "ripe" or "none" (not a plot).
    pub care: String,
    pub barrel: Barrel,
    pub refill_ready: bool,
    pub plant_food: u32,
    pub food_max: u32,
    pub feed_ready: bool,
    /// "fed", "empty" (no plant food), "feed" or "none".
    pub feeding: String,
}

#[derive(Default, Data)]
pub struct ShopRow {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub price: u64,
    pub stock: u32,
    pub owned: u32,
    pub affordable: bool,
    pub held: bool,
    pub grow_s: u32,
    /// Zero for a single harvest.
    pub fruit_s: u32,
}

#[derive(Default, Data)]
pub struct ShopHud {
    pub shop: Vec<ShopRow>,
}

#[derive(Default, Data)]
pub struct BagRow {
    pub id: String,
    pub fruit: Described,
    pub weight: f32,
    pub value: u64,
}

#[derive(Default, Data)]
pub struct BagHud {
    pub bag: Vec<BagRow>,
    pub bag_count: u32,
    pub bag_value: u64,
    pub bag_page: u32,
    pub bag_pages: u32,
}

/// Backpack rows per published page. The whole record may be 16 MiB and a
/// change re-encodes only its field, but a 164,000-fruit backpack is still a
/// 13.6 MB field rebuilt on every harvest (311 ms); a page of 200 is 18 KB
/// (18 ms), and is the size Grow a Garden's own backpack holds.
pub const BAG_PAGE: usize = 200;

/// Publishes what changed.
pub fn publish(w: &World, force: bool) {
    let (shop_dirty, bag_dirty) = {
        let mut farm = w.resource_mut::<Farm>();
        let d = (farm.shop_dirty || force, farm.bag_dirty || force);
        farm.shop_dirty = false;
        farm.bag_dirty = false;
        d
    };
    let b = balance(w);
    let farm = w.resource::<Farm>();
    if shop_dirty {
        let shop = w.resource::<Shop>();
        let rows = b
            .crops
            .iter()
            .enumerate()
            .map(|(k, c)| ShopRow {
                id: c.id.clone(),
                name: c.name.clone(),
                rarity: c.rarity.clone(),
                price: c.price,
                stock: shop.stock.get(k).copied().unwrap_or(0),
                owned: farm.seeds[k],
                affordable: farm.sheckles >= c.price,
                held: farm.held == Some(k as u8),
                grow_s: c.grow_s,
                fruit_s: c.fruit_s,
            })
            .collect();
        w.publish_record(&ShopHud { shop: rows });
    }
    if bag_dirty {
        let pages = farm.bag.len().div_ceil(BAG_PAGE).max(1);
        let page = (farm.bag_page as usize).min(pages - 1);
        let rows = farm
            .bag
            .iter()
            .rev()
            .skip(page * BAG_PAGE)
            .take(BAG_PAGE)
            .map(|i| BagRow {
                id: i.id.to_string(),
                fruit: Described::of(&b, i),
                weight: i.weight,
                value: i.value(&b),
            })
            .collect();
        w.publish_record(&BagHud {
            bag: rows,
            bag_count: farm.bag.len() as u32,
            bag_value: farm.bag.iter().map(|i| i.value(&b)).sum(),
            bag_page: page as u32 + 1,
            bag_pages: pages as u32,
        });
    }
    drop(farm);
    w.publish_record(&status(w));
}

/// The status record: what the player's tile, purse, market, care and the
/// garden say now.
pub fn status(w: &World) -> Status {
    let now = now_ms(w);
    let b = balance(w);
    let (tile, prompt) = farm::prompt(w);
    let farm = w.resource::<Farm>();
    let census = w.resource::<Census>();
    let weather = w.resource::<Weather>();
    let shop = w.resource::<Shop>();
    let schedule = w.resource::<Schedule>();
    let order = b.order(farm.orders);
    let carried = order
        .map(|(kind, _, _)| farm.bag.iter().filter(|i| i.kind == kind).count() as u32)
        .unwrap_or(0);
    let target = tile.and_then(|tile| farm.at(tile));
    let planted = target.and_then(|e| w.get::<Plant>(e).map(|p| p.kind));
    let needs_water = target.is_some_and(|p| crate::garden::needs_water(w, p));
    let order_hint = match order {
        None => Hint {
            what: "explore".into(),
            ..Hint::default()
        },
        Some((kind, count, _)) => {
            let c = b.crop(kind);
            let hint = |what: &str| Hint {
                what: what.into(),
                crop: c.name.clone(),
                ..Hint::default()
            };
            if carried >= count {
                hint("deliver")
            } else if planted == Some(kind) {
                hint(if prompt.what == "harvest" {
                    "harvest_here"
                } else {
                    "wait_here"
                })
            } else if farm.seeds[kind as usize] > 0 {
                if farm.held != Some(kind) {
                    hint("hold_seed")
                } else if tile.is_none() {
                    hint("walk_on")
                } else if let Some(other) = planted {
                    Hint {
                        other: b.crop(other).name.clone(),
                        ..hint("move")
                    }
                } else {
                    hint("plant_here")
                }
            } else if shop.stock[kind as usize] == 0 {
                Hint {
                    s: secs(shop.next.saturating_sub(now)),
                    ..hint("sold_out")
                }
            } else if farm.sheckles < c.price {
                Hint {
                    price: c.price,
                    ..hint("costs")
                }
            } else {
                hint("buy")
            }
        }
    };
    Status {
        sheckles: farm.sheckles,
        held: farm.held.map(|k| b.crop(k).id.clone()).unwrap_or_default(),
        held_name: farm
            .held
            .map(|k| b.crop(k).name.clone())
            .unwrap_or_default(),
        held_count: farm.held.map_or(0, |k| farm.seeds[k as usize]),
        prompt,
        plot: tile
            .map(|[x, z]| Plot {
                inside: true,
                x: x as u32,
                z: z as u32,
                crop: planted.map(|k| b.crop(k).name.clone()).unwrap_or_default(),
            })
            .unwrap_or_default(),
        planting: farm::planting_guidance(w),
        weather: match weather.sky {
            Sky::Clear => "clear",
            Sky::Rain => "rain",
            Sky::Snow => "snow",
            Sky::Storm => "storm",
        }
        .into(),
        weather_s: if weather.sky == Sky::Clear {
            0
        } else {
            secs(weather.until.saturating_sub(now))
        },
        restock_s: secs(shop.next.saturating_sub(now)),
        plants: census.plants,
        fruit: census.fruit,
        ripe: census.ripe,
        mutated: census.mutated,
        size: farm.size as u32,
        expand_cost: farm.expand_cost(&b),
        last: farm.last.clone(),
        away: farm.away.clone(),
        garden_s: secs(now),
        events: schedule.processed as f64,
        queued: schedule.heap.len() as u32,
        order: Order {
            n: farm.orders,
            total: b.orders.len() as u32,
            done: order.is_none(),
            crop: order
                .map(|(kind, _, _)| b.crop(kind).name.clone())
                .unwrap_or_default(),
            count: order.map_or(0, |(_, count, _)| count),
            bonus: order.map_or(0, |(_, _, bonus)| bonus),
            carried,
        },
        order_hint,
        order_seed: order
            .filter(|&(kind, count, _)| {
                carried < count
                    && planted != Some(kind)
                    && farm.seeds[kind as usize] > 0
                    && farm.held != Some(kind)
            })
            .map(|(kind, _, _)| b.crop(kind).id.clone())
            .unwrap_or_default(),
        order_ready: order.is_some_and(|(_, count, _)| carried >= count),
        orders: farm.orders,
        water: farm.water as u32,
        water_max: b.farm.water as u32,
        water_ready: farm.water > 0 && needs_water,
        care: if farm.water == 0 {
            "empty"
        } else if needs_water {
            "water"
        } else if target.is_some_and(|p| is_growing(w, p)) {
            "watered"
        } else if target.is_some() {
            "ripe"
        } else {
            "none"
        }
        .into(),
        barrel: farm::barrel(w),
        refill_ready: farm.water < b.farm.water && farm::at_barrel(w),
        plant_food: farm.plant_food as u32,
        food_max: b.farm.food as u32,
        feed_ready: farm.plant_food > 0 && target.is_some_and(|p| crate::garden::needs_feed(w, p)),
        feeding: if target.is_some_and(|p| crate::garden::is_fed(w, p)) {
            "fed"
        } else if farm.plant_food == 0 {
            "empty"
        } else if target.is_some_and(|p| crate::garden::needs_feed(w, p)) {
            "feed"
        } else {
            "none"
        }
        .into(),
    }
}

fn is_growing(w: &World, e: Entity) -> bool {
    w.get::<Plant>(e).is_some_and(|p| p.stage < 4)
        || crate::garden::fruits_of(w, e).iter().any(|f| !f.1)
}
