//! The seed shop: stock rolls on a restock (every five minutes, by the
//! balance), rarer seeds less often.

use crate::crops::balance;
use crate::garden::{Due, Schedule};
use exact_game::*;

#[derive(Default, Resource)]
pub struct Shop {
    /// Stock by crop kind.
    pub stock: Vec<u32>,
    pub next: u64,
    pub restocks: u32,
}

pub fn restock(w: &mut World, at: u64) {
    let b = balance(w);
    let mut stock: Vec<u32> = {
        let mut rng = w.rng();
        b.crops
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
    if let Some((kind, _, _)) = b.order(w.resource::<crate::farm::Farm>().orders) {
        stock[kind as usize] = stock[kind as usize].max(1);
    }
    let every = b.farm.restock_s * 1000;
    {
        let mut shop = w.resource_mut::<Shop>();
        shop.stock = stock;
        shop.next = at + every;
        shop.restocks += 1;
    }
    w.resource_mut::<Schedule>().push(at + every, Due::Restock);
    w.resource_mut::<crate::farm::Farm>().shop_dirty = true;
}

/// Buys one seed of `kind`; refuses with the reason.
pub fn buy(w: &World, kind: u8) -> Result<(), crate::hud::Note> {
    use crate::hud::Note;
    let b = balance(w);
    let c = b.crop(kind);
    let mut shop = w.resource_mut::<Shop>();
    let mut farm = w.resource_mut::<crate::farm::Farm>();
    if shop.stock[kind as usize] == 0 {
        return Err(Note::new("out_of_stock").crop(&c.name));
    }
    if farm.sheckles < c.price {
        return Err(Note::new("costs").crop(&c.name).coins(c.price));
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
