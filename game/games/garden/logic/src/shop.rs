//! The seed shop: stock rolls on a five-minute restock, rarer seeds less often.

use crate::crops::{crop, CROPS};
use crate::garden::{Due, Schedule};
use exact_game::*;

pub const RESTOCK_MS: u64 = 300_000;

#[derive(Default, Resource)]
pub struct Shop {
    /// Stock by crop kind.
    pub stock: Vec<u32>,
    pub next: u64,
    pub restocks: u32,
}

pub fn restock(w: &mut World, at: u64) {
    let mut stock: Vec<u32> = {
        let mut rng = w.rng();
        CROPS
            .iter()
            .map(|c| {
                if rng.chance(c.appear) {
                    rng.range(1..c.max_stock + 1)
                } else {
                    0
                }
            })
            .collect()
    };
    if let Some(&(kind, _, _)) =
        crate::farm::ORDERS.get(w.resource::<crate::farm::Farm>().orders as usize)
    {
        stock[kind as usize] = stock[kind as usize].max(1);
    }
    {
        let mut shop = w.resource_mut::<Shop>();
        shop.stock = stock;
        shop.next = at + RESTOCK_MS;
        shop.restocks += 1;
    }
    w.resource_mut::<Schedule>()
        .push(at + RESTOCK_MS, Due::Restock);
    w.resource_mut::<crate::farm::Farm>().shop_dirty = true;
}

/// Buys one seed of `kind`; refuses with the reason.
pub fn buy(w: &World, kind: u8) -> Result<(), String> {
    let c = crop(kind);
    let mut shop = w.resource_mut::<Shop>();
    let mut farm = w.resource_mut::<crate::farm::Farm>();
    if shop.stock[kind as usize] == 0 {
        return Err(format!("{} is out of stock", c.name));
    }
    if farm.sheckles < c.price {
        return Err(format!("{} costs {}¢", c.name, c.price));
    }
    shop.stock[kind as usize] -= 1;
    farm.sheckles -= c.price;
    farm.seeds[kind as usize] += 1;
    if farm.held.is_none() {
        farm.held = Some(kind);
    }
    farm.shop_dirty = true;
    Ok(())
}
