//! What the HUD reads. Three records, each published only when it changes:
//! the status line (at most once a garden second, for its timers), the shop
//! and seeds, and the backpack.

use crate::crops::{self, compact, crop, CROPS};
use crate::farm::Farm;
use crate::garden::{now_ms, Census, Schedule, Weather};
use crate::shop::Shop;
use exact_game::*;

#[derive(Default, Data)]
pub struct Status {
    pub sheckles: String,
    pub sheckles_n: f64,
    pub held: String,
    pub prompt: String,
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
    pub clock: String,
    pub night: bool,
    pub events: f64,
    pub queued: u32,
}

#[derive(Default, Data)]
pub struct ShopRow {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub color: String,
    pub fruit: String,
    pub price: String,
    pub stock: u32,
    pub owned: u32,
    pub affordable: bool,
    pub held: bool,
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
                fruit: format!(
                    "#{:02x}{:02x}{:02x}",
                    (c.fruit[0] * 255.0) as u8,
                    (c.fruit[1] * 255.0) as u8,
                    (c.fruit[2] * 255.0) as u8
                ),
                price: compact(c.price),
                stock: shop.stock.get(k).copied().unwrap_or(0),
                owned: farm.seeds[k],
                affordable: farm.sheckles >= c.price,
                held: farm.held == Some(k as u8),
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
    w.publish_record(&Status {
        sheckles: compact(farm.sheckles),
        sheckles_n: farm.sheckles as f64,
        held: farm
            .held
            .map(|k| format!("{} ×{}", crop(k).name, farm.seeds[k as usize]))
            .unwrap_or_default(),
        prompt,
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
        clock: crate::look::clock_label(now),
        night: crate::look::day_phase(now) > 0.5,
        events: schedule.processed as f64,
        queued: schedule.heap.len() as u32,
    });
}
