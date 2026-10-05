//! The balance: the seed catalogue, mutations, market orders, weather odds and
//! the farm's rules, authored in `assets/garden.level.json` and read through
//! the engine's typed data path (`Game::LEVELS`). It is simulation data: a save
//! records its content identity, so a balance change moves the pins.

use exact_game::*;
use std::sync::Arc;

/// The balance's asset name (`assets/garden.level.json`).
pub const BALANCE: &str = "garden.level.json";

/// One kind of seed: price, growth and the fruit it bears.
#[derive(Clone, Default, Debug, Data)]
pub struct Crop {
    pub id: String,
    pub name: String,
    /// "Common" … "Divine": how rare a seed is in the shop.
    pub rarity: String,
    pub price: u64,
    /// Seconds from planting to the first fruit.
    pub grow_s: u32,
    /// Seconds for a fruit to ripen (and to regrow after a harvest); zero for
    /// a single-harvest crop, whose one fruit ripens as it appears.
    pub fruit_s: u32,
    /// Fruit slots on a mature plant. A single-harvest crop has one, and
    /// harvesting it removes the plant.
    pub slots: u8,
    /// Sheckles for a normal fruit at base weight.
    pub value: u64,
    /// Base weight in kilograms; a fruit's value scales with weight².
    pub weight: f32,
    /// Chance in a restock that the seed appears, and its most stock.
    pub appear: f32,
    pub max_stock: u32,
    /// Plant height when mature, and its colours.
    pub height: f32,
    pub leaf: [f32; 3],
    pub fruit: [f32; 3],
    pub fruit_size: f32,
}
impl Crop {
    pub fn regrows(&self) -> bool {
        self.fruit_s > 0
    }
}

/// A mutation, in bit order (`GOLD`, `RAINBOW`, `WET`, …): its name, its value
/// multiplier, and the colour and glow of the classic look's fruit.
#[derive(Clone, Default, Debug, Data)]
pub struct Mutation {
    pub name: String,
    pub multiplier: f64,
    pub color: [f32; 3],
    pub glow: f32,
}

/// The rolls a ripening fruit makes: thresholds on one draw for the growth
/// variants, a chance for each weather's mutations, and its weight.
#[derive(Clone, Default, Debug, Data)]
pub struct Ripening {
    /// A draw below this is Rainbow; below `gold_below`, Gold.
    pub rainbow_below: f32,
    pub gold_below: f32,
    pub rain_wet: f32,
    /// Chilled, or failing that Frozen.
    pub snow_chilled: f32,
    pub snow_frozen: f32,
    pub storm_wet: f32,
    pub storm_shocked: f32,
    /// The base weight's multiplier, drawn in [low, high).
    pub weight: [f32; 2],
    /// A giant's chance and multiplier; a fed fruit's multiplier.
    pub giant: f32,
    pub giant_weight: f32,
    pub fed_weight: f32,
}

/// What weather comes, and for how long (seconds, both ends inclusive).
#[derive(Clone, Default, Debug, Data)]
pub struct Skies {
    /// A draw below this brings rain; below `snow_below`, snow; else a storm.
    pub rain_below: f32,
    pub snow_below: f32,
    pub stormy_s: [u64; 2],
    pub clear_s: [u64; 2],
}

/// A market request: crop, quantity, bonus on top of value.
#[derive(Clone, Default, Debug, Data)]
pub struct Order {
    pub crop: String,
    pub count: u32,
    pub bonus: u64,
}

#[derive(Clone, Default, Debug, Data)]
pub struct Rules {
    pub start_size: u16,
    pub max_size: u16,
    pub start_sheckles: u64,
    /// Watering-can and plant-food doses.
    pub water: u8,
    pub food: u8,
    /// Expanding costs this × size³.
    pub expand_cost: u64,
    pub restock_s: u64,
}

/// The following camera: its offset and lag, and the overview's (its height
/// and distance as fractions of the garden's span, at least `overview_min`).
#[derive(Clone, Default, Debug, Data)]
pub struct CameraRig {
    pub offset: [f32; 3],
    pub lag: f32,
    pub overview: [f32; 2],
    pub overview_lag: f32,
    pub overview_min: f32,
}

#[derive(Clone, Default, Debug, Data)]
pub struct Balance {
    /// Every seed, cheapest first. The index is the saved kind.
    pub crops: Vec<Crop>,
    pub mutations: Vec<Mutation>,
    pub ripening: Ripening,
    pub weather: Skies,
    pub orders: Vec<Order>,
    pub farm: Rules,
    pub camera: CameraRig,
}

/// The garden's balance, decoded once when it arrived.
pub fn balance(w: &World) -> Arc<Balance> {
    w.level::<Balance>(BALANCE).expect("the declared balance")
}

/// The balance from `Game::present`.
pub fn shown_balance(p: &Present) -> Arc<Balance> {
    p.level::<Balance>(BALANCE).expect("the declared balance")
}

/// Mutation bits on a fruit. Gold and Rainbow are growth variants (at most one);
/// the rest are environmental and stack.
pub const GOLD: u8 = 1;
pub const RAINBOW: u8 = 2;
pub const WET: u8 = 4;
pub const CHILLED: u8 = 8;
pub const SHOCKED: u8 = 16;
pub const FROZEN: u8 = 32;

impl Balance {
    pub fn crop(&self, kind: u8) -> &Crop {
        &self.crops[kind as usize]
    }

    pub fn kind_of(&self, id: &str) -> Option<u8> {
        self.crops.iter().position(|c| c.id == id).map(|i| i as u8)
    }

    /// The `i`th market request as (crop kind, quantity, bonus).
    pub fn order(&self, i: u32) -> Option<(u8, u32, u64)> {
        let o = self.orders.get(i as usize)?;
        Some((self.kind_of(&o.crop)?, o.count, o.bonus))
    }

    fn mutation(&self, bit: u8) -> &Mutation {
        &self.mutations[bit.trailing_zeros() as usize]
    }

    /// The names of a fruit's mutations, in bit order.
    pub fn mutation_names(&self, bits: u8) -> Vec<&str> {
        (0..self.mutations.len())
            .filter(|&i| bits & (1 << i) != 0)
            .map(|i| self.mutations[i].name.as_str())
            .collect()
    }

    /// Grow a Garden's rule: base × weight² × variant × (1 + Σ(environment − 1)).
    pub fn fruit_value(&self, kind: u8, weight: f32, bits: u8) -> u64 {
        let c = self.crop(kind);
        let w = (weight / c.weight) as f64;
        let variant = if bits & RAINBOW != 0 {
            self.mutation(RAINBOW).multiplier
        } else if bits & GOLD != 0 {
            self.mutation(GOLD).multiplier
        } else {
            1.0
        };
        let env: f64 = (0..self.mutations.len())
            .filter(|&i| (1u8 << i) > RAINBOW && bits & (1 << i) != 0)
            .map(|i| self.mutations[i].multiplier - 1.0)
            .sum();
        (c.value as f64 * w * w * variant * (1.0 + env))
            .round()
            .max(1.0) as u64
    }

    /// The colour a fruit shows in the classic look: its crop's, or its
    /// strongest mutation's.
    pub fn fruit_color(&self, kind: u8, bits: u8) -> [f32; 3] {
        for bit in [RAINBOW, GOLD, SHOCKED, FROZEN] {
            if bits & bit != 0 {
                return self.mutation(bit).color;
            }
        }
        let c = self.crop(kind).fruit;
        if bits & (WET | CHILLED) != 0 {
            [c[0] * 0.6 + 0.1, c[1] * 0.6 + 0.2, c[2] * 0.6 + 0.4]
        } else {
            c
        }
    }

    /// How brightly a ripe fruit glows: its brightest mutation's glow.
    pub fn fruit_glow(&self, bits: u8) -> f32 {
        (0..self.mutations.len())
            .filter(|&i| bits & (1 << i) != 0)
            .map(|i| self.mutations[i].glow)
            .fold(0., f32::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn balance() -> Balance {
        exact_game::json::from_str(include_str!("../../assets/garden.level.json")).unwrap()
    }

    #[test]
    fn values_follow_weight_and_mutations() {
        let b = balance();
        let carrot = b.kind_of("carrot").unwrap();
        assert_eq!(b.fruit_value(carrot, 0.25, 0), 18);
        assert_eq!(b.fruit_value(carrot, 0.5, 0), 72);
        assert_eq!(b.fruit_value(carrot, 0.25, GOLD), 360);
        assert_eq!(b.fruit_value(carrot, 0.25, GOLD | WET), 720);
        assert_eq!(b.fruit_value(carrot, 0.25, RAINBOW | WET | CHILLED), 2700);
        assert_eq!(b.mutation_names(GOLD | WET), ["Gold", "Wet"]);
    }

    #[test]
    fn catalogue_is_sorted_and_whole() {
        let b = balance();
        assert!(b.crops.windows(2).all(|w| w[0].price <= w[1].price));
        assert!(b
            .crops
            .iter()
            .all(|c| c.slots >= 1 && (c.regrows() || c.slots == 1)));
        assert_eq!(b.mutations.len(), 6);
        assert!((0..b.orders.len() as u32).all(|i| b.order(i).is_some()));
    }
}
