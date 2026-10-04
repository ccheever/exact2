//! What the HUD reads. Three records, each published only when it changes:
//! the status line (at most once a garden second, for its timers), the shop
//! and seeds, and the backpack.

use crate::crops::{self, compact, crop, CROPS};
use crate::farm::Farm;
use crate::garden::{now_ms, Census, Plant, Schedule, Weather};
use crate::shop::Shop;
use exact_game::*;

#[derive(Default, Data)]
pub struct Status {
    pub sheckles: String,
    pub sheckles_n: f64,
    pub held: String,
    pub prompt: String,
    pub plot: String,
    pub planting: String,
    pub weather: String,
    pub weather_left: String,
    pub restock_in: String,
    pub plants: u32,
    pub fruit: u32,
    pub ripe: u32,
    pub mutated: u32,
    pub size: u32,
    pub expand_cost: String,
    pub last: String,
    pub away: String,
    pub garden_time: String,
    pub events: f64,
    pub queued: u32,
    pub order: String,
    pub order_detail: String,
    pub order_hint: String,
    pub order_seed: String,
    pub order_ready: bool,
    pub orders: u32,
    pub water: u32,
    pub water_ready: bool,
    pub care: String,
    pub refill: String,
    pub refill_ready: bool,
}

#[derive(Default, Data)]
pub struct ShopRow {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub color: String,
    pub price: String,
    pub stock: u32,
    pub owned: u32,
    pub affordable: bool,
    pub held: bool,
    pub growth: String,
}

#[derive(Default, Data)]
pub struct ShopHud {
    pub shop: Vec<ShopRow>,
}

#[derive(Default, Data)]
pub struct BagRow {
    pub id: String,
    pub label: String,
    pub weight: String,
    pub value: String,
    pub mutated: bool,
}

#[derive(Default, Data)]
pub struct BagHud {
    pub bag: Vec<BagRow>,
    pub bag_count: u32,
    pub bag_value: String,
    pub bag_page: u32,
    pub bag_pages: u32,
}

/// Backpack rows per published page. The whole record may be 16 MiB and a
/// change re-encodes only its field, but a 164,000-fruit backpack is still a
/// 13.6 MB field rebuilt on every harvest (311 ms); a page of 200 is 18 KB
/// (18 ms), and is the size Grow a Garden's own backpack holds.
pub const BAG_PAGE: usize = 200;

/// Publishes what changed. `prompt` is the player's tile text.
pub fn publish(w: &World, prompt: String, force: bool) {
    let now = now_ms(w);
    let (shop_dirty, bag_dirty) = {
        let mut farm = w.resource_mut::<Farm>();
        let d = (farm.shop_dirty || force, farm.bag_dirty || force);
        farm.shop_dirty = false;
        farm.bag_dirty = false;
        d
    };
    let farm = w.resource::<Farm>();
    if shop_dirty {
        let shop = w.resource::<Shop>();
        let rows = CROPS
            .iter()
            .enumerate()
            .map(|(k, c)| ShopRow {
                id: c.id.into(),
                name: c.name.into(),
                rarity: c.rarity.name().into(),
                color: c.rarity.color().into(),
                price: compact(c.price),
                stock: shop.stock.get(k).copied().unwrap_or(0),
                owned: farm.seeds[k],
                affordable: farm.sheckles >= c.price,
                held: farm.held == Some(k as u8),
                growth: if c.regrows {
                    format!(
                        "{}s to first fruit · regrows every {}s",
                        c.grow_s + c.fruit_s,
                        c.fruit_s
                    )
                } else {
                    format!("{}s to harvest · one harvest", c.grow_s)
                },
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
                label: i.label(),
                weight: format!("{:.2} kg", i.weight),
                value: compact(i.value()),
                mutated: i.muts != 0,
            })
            .collect();
        let total: u64 = farm.bag.iter().map(|i| i.value()).sum();
        w.publish_record(&BagHud {
            bag: rows,
            bag_count: farm.bag.len() as u32,
            bag_value: compact(total),
            bag_page: page as u32 + 1,
            bag_pages: pages as u32,
        });
    }
    let census = w.resource::<Census>();
    let weather = w.resource::<Weather>();
    let shop = w.resource::<Shop>();
    let schedule = w.resource::<Schedule>();
    let order = crate::farm::ORDERS.get(farm.orders as usize);
    let carried = order
        .map(|&(kind, _, _)| farm.bag.iter().filter(|i| i.kind == kind).count() as u32)
        .unwrap_or(0);
    let tile = w
        .global_position("player")
        .and_then(|p| crate::farm::tile_at(w, p));
    let planted = tile
        .and_then(|tile| farm.at(tile))
        .and_then(|e| w.get::<Plant>(e).map(|p| p.kind));
    let target = tile.and_then(|tile| farm.at(tile));
    let needs_water = target.is_some_and(|p| crate::garden::needs_water(w, p));
    let order_hint = order
        .map(|&(kind, count, _)| {
            let c = crop(kind);
            if carried >= count {
                "Deliver the fruit in your backpack to collect the bonus.".into()
            } else if planted == Some(kind) {
                if prompt.starts_with("E: harvest") {
                    format!("Press E to harvest the {} here.", c.name)
                } else {
                    format!("Wait here for {} to ripen, then harvest with E.", c.name)
                }
            } else if farm.seeds[kind as usize] > 0 {
                if farm.held != Some(kind) {
                    format!("Hold your {} seed, then find an empty tile.", c.name)
                } else if tile.is_none() {
                    format!("Walk onto the garden to plant {}.", c.name)
                } else if let Some(other) = planted {
                    format!(
                        "This tile has {}. Move to an empty tile to plant {}.",
                        crop(other).name,
                        c.name
                    )
                } else {
                    format!("Press E to plant {} on this empty tile.", c.name)
                }
            } else if shop.stock[kind as usize] == 0 {
                format!(
                    "{} seeds are sold out. Restock in {}.",
                    c.name,
                    crops::clock(shop.next.saturating_sub(now))
                )
            } else if farm.sheckles < c.price {
                format!(
                    "{} costs {}¢. Harvest and sell spare fruit to earn more.",
                    c.name,
                    compact(c.price)
                )
            } else {
                format!(
                    "Buy a {} seed in the shop, or return to a {} plant.",
                    c.name, c.name
                )
            }
        })
        .unwrap_or_else(|| "Try rare seeds, mutations and a bigger garden.".into());
    w.publish_record(&Status {
        sheckles: compact(farm.sheckles),
        sheckles_n: farm.sheckles as f64,
        held: farm
            .held
            .map(|k| format!("{} ×{}", crop(k).name, farm.seeds[k as usize]))
            .unwrap_or_default(),
        prompt,
        plot: tile
            .map(|[x, z]| {
                format!(
                    "Plot {}, {} · {}",
                    x + 1,
                    z + 1,
                    planted.map(|k| crop(k).name).unwrap_or("Empty")
                )
            })
            .unwrap_or_else(|| "Outside the garden".into()),
        planting: crate::farm::planting_guidance(w),
        weather: weather.sky.name().into(),
        weather_left: if weather.sky == crate::garden::Sky::Clear {
            String::new()
        } else {
            crops::clock(weather.until.saturating_sub(now))
        },
        restock_in: crops::clock(shop.next.saturating_sub(now)),
        plants: census.plants,
        fruit: census.fruit,
        ripe: census.ripe,
        mutated: census.mutated,
        size: farm.size as u32,
        expand_cost: compact(farm.expand_cost()),
        last: farm.last.clone(),
        away: farm.away.clone(),
        garden_time: crops::clock(now),
        events: schedule.processed as f64,
        queued: schedule.heap.len() as u32,
        order: order
            .map(|&(kind, count, _)| {
                format!(
                    "Market order {} · {count} {}",
                    farm.orders + 1,
                    crop(kind).name
                )
            })
            .unwrap_or_else(|| "Market regular · all 5 orders filled!".into()),
        order_detail: order
            .map(|&(_, count, bonus)| {
                format!("{carried}/{count} in backpack · full value + {bonus}¢ bonus")
            })
            .unwrap_or_else(|| "Keep growing, find mutations, expand your garden".into()),
        order_hint,
        order_seed: order
            .filter(|&&(kind, count, _)| {
                carried < count
                    && planted != Some(kind)
                    && farm.seeds[kind as usize] > 0
                    && farm.held != Some(kind)
            })
            .map(|&(kind, _, _)| crop(kind).id.into())
            .unwrap_or_default(),
        order_ready: order.is_some_and(|&(_, count, _)| carried >= count),
        orders: farm.orders,
        water: farm.water as u32,
        water_ready: farm.water > 0 && needs_water,
        care: if farm.water == 0 {
            "Can empty · refill at the blue barrel".into()
        } else if needs_water {
            "Water this plot · remaining wait −25%".into()
        } else if target.is_some_and(|p| is_growing(w, p)) {
            "Watered · growing faster".into()
        } else if target.is_some() {
            "Harvest ripe fruit before watering".into()
        } else {
            "Stand on a growing plot to water".into()
        },
        refill: crate::farm::refill_guidance(w),
        refill_ready: farm.water < crate::farm::WATER_CAPACITY && crate::farm::at_barrel(w),
    });
}

fn is_growing(w: &World, e: Entity) -> bool {
    w.get::<Plant>(e).is_some_and(|p| p.stage < 4)
        || crate::garden::fruits_of(w, e).iter().any(|f| !f.1)
}
