//! The seed catalogue, mutations and what a fruit is worth.

/// How rare a seed is in the shop.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
    Mythical,
    Divine,
}

impl Rarity {
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Legendary => "Legendary",
            Rarity::Mythical => "Mythical",
            Rarity::Divine => "Divine",
        }
    }
    pub fn color(self) -> &'static str {
        match self {
            Rarity::Common => "#9ca3af",
            Rarity::Uncommon => "#4ade80",
            Rarity::Rare => "#60a5fa",
            Rarity::Legendary => "#facc15",
            Rarity::Mythical => "#c084fc",
            Rarity::Divine => "#fb923c",
        }
    }
}

/// One kind of seed: price, growth and the fruit it bears.
pub struct Crop {
    pub id: &'static str,
    pub name: &'static str,
    pub rarity: Rarity,
    pub price: u64,
    /// Seconds from planting to the first fruit.
    pub grow_s: u32,
    /// Seconds for a fruit to ripen (and to regrow after a harvest).
    pub fruit_s: u32,
    /// Fruit slots on a mature plant. A single-harvest crop has one, and
    /// harvesting it removes the plant.
    pub slots: u8,
    pub regrows: bool,
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

macro_rules! crop {
    ($id:literal, $name:literal, $rarity:ident, $price:literal, $grow:literal, $fruit_s:literal, $slots:literal, $regrows:literal,
     $value:literal, $weight:literal, $appear:literal, $stock:literal, $height:literal, $leaf:expr, $fruit:expr, $size:literal) => {
        Crop {
            id: $id,
            name: $name,
            rarity: Rarity::$rarity,
            price: $price,
            grow_s: $grow,
            fruit_s: $fruit_s,
            slots: $slots,
            regrows: $regrows,
            value: $value,
            weight: $weight,
            appear: $appear,
            max_stock: $stock,
            height: $height,
            leaf: $leaf,
            fruit: $fruit,
            fruit_size: $size,
        }
    };
}

/// Every seed, cheapest first. The index is the saved kind.
pub const CROPS: &[Crop] = &[
    crop!(
        "carrot",
        "Carrot",
        Common,
        10,
        20,
        0,
        1,
        false,
        18,
        0.25,
        1.0,
        25,
        0.4,
        [0.3, 0.7, 0.25],
        [0.95, 0.5, 0.1],
        0.22
    ),
    crop!(
        "strawberry",
        "Strawberry",
        Common,
        50,
        40,
        30,
        4,
        true,
        14,
        0.25,
        0.9,
        12,
        0.5,
        [0.25, 0.6, 0.25],
        [0.9, 0.12, 0.15],
        0.14
    ),
    crop!(
        "blueberry",
        "Blueberry",
        Uncommon,
        400,
        60,
        40,
        5,
        true,
        40,
        0.18,
        0.7,
        8,
        0.6,
        [0.2, 0.5, 0.3],
        [0.25, 0.3, 0.85],
        0.12
    ),
    crop!(
        "tomato",
        "Tomato",
        Rare,
        800,
        90,
        60,
        4,
        true,
        60,
        0.45,
        0.5,
        6,
        0.9,
        [0.25, 0.55, 0.2],
        [0.85, 0.15, 0.1],
        0.2
    ),
    crop!(
        "corn",
        "Corn",
        Rare,
        1300,
        120,
        80,
        3,
        true,
        75,
        0.6,
        0.4,
        5,
        1.4,
        [0.45, 0.65, 0.2],
        [0.95, 0.85, 0.25],
        0.18
    ),
    crop!(
        "watermelon",
        "Watermelon",
        Rare,
        2500,
        180,
        0,
        1,
        false,
        2700,
        7.0,
        0.3,
        4,
        0.5,
        [0.2, 0.5, 0.2],
        [0.2, 0.6, 0.25],
        0.55
    ),
    crop!(
        "pumpkin",
        "Pumpkin",
        Legendary,
        3000,
        240,
        0,
        1,
        false,
        3400,
        6.0,
        0.25,
        3,
        0.5,
        [0.3, 0.5, 0.2],
        [0.95, 0.55, 0.1],
        0.5
    ),
    crop!(
        "apple",
        "Apple",
        Legendary,
        3250,
        300,
        90,
        6,
        true,
        270,
        2.8,
        0.2,
        3,
        2.4,
        [0.2, 0.45, 0.2],
        [0.8, 0.1, 0.12],
        0.2
    ),
    crop!(
        "bamboo",
        "Bamboo",
        Legendary,
        4000,
        200,
        0,
        1,
        false,
        4000,
        3.8,
        0.2,
        5,
        2.8,
        [0.45, 0.75, 0.3],
        [0.5, 0.8, 0.35],
        0.25
    ),
    crop!(
        "coconut",
        "Coconut",
        Mythical,
        6000,
        420,
        120,
        3,
        true,
        400,
        14.0,
        0.12,
        2,
        3.2,
        [0.25, 0.5, 0.2],
        [0.45, 0.3, 0.15],
        0.28
    ),
    crop!(
        "cactus",
        "Cactus",
        Mythical,
        15000,
        360,
        150,
        3,
        true,
        3400,
        7.0,
        0.1,
        2,
        1.8,
        [0.3, 0.6, 0.35],
        [0.9, 0.4, 0.6],
        0.2
    ),
    crop!(
        "dragon",
        "Dragon Fruit",
        Mythical,
        50000,
        600,
        180,
        4,
        true,
        4750,
        11.0,
        0.06,
        2,
        1.6,
        [0.35, 0.6, 0.3],
        [0.95, 0.2, 0.55],
        0.24
    ),
    crop!(
        "mango",
        "Mango",
        Mythical,
        100000,
        720,
        200,
        4,
        true,
        6300,
        14.0,
        0.04,
        1,
        2.6,
        [0.2, 0.5, 0.2],
        [1.0, 0.65, 0.15],
        0.24
    ),
    crop!(
        "grape",
        "Grape",
        Divine,
        850000,
        900,
        240,
        6,
        true,
        7850,
        3.0,
        0.01,
        1,
        1.4,
        [0.25, 0.45, 0.25],
        [0.45, 0.15, 0.55],
        0.16
    ),
];

pub fn crop(kind: u8) -> &'static Crop {
    &CROPS[kind as usize]
}

pub fn kind_of(id: &str) -> Option<u8> {
    CROPS.iter().position(|c| c.id == id).map(|i| i as u8)
}

/// Mutation bits on a fruit. Gold and Rainbow are growth variants (at most one);
/// the rest are environmental and stack.
pub const GOLD: u8 = 1;
pub const RAINBOW: u8 = 2;
pub const WET: u8 = 4;
pub const CHILLED: u8 = 8;
pub const SHOCKED: u8 = 16;
pub const FROZEN: u8 = 32;

/// (bit, name, multiplier)
pub const MUTATIONS: &[(u8, &str, f64)] = &[
    (GOLD, "Gold", 20.0),
    (RAINBOW, "Rainbow", 50.0),
    (WET, "Wet", 2.0),
    (CHILLED, "Chilled", 2.0),
    (SHOCKED, "Shocked", 100.0),
    (FROZEN, "Frozen", 10.0),
];

pub fn mutation_names(bits: u8) -> String {
    let names: Vec<&str> = MUTATIONS
        .iter()
        .filter(|(b, _, _)| bits & b != 0)
        .map(|(_, n, _)| *n)
        .collect();
    names.join(" ")
}

/// Grow a Garden's rule: base × weight² × variant × (1 + Σ(environment − 1)).
pub fn fruit_value(kind: u8, weight: f32, bits: u8) -> u64 {
    let c = crop(kind);
    let w = (weight / c.weight) as f64;
    let variant = if bits & RAINBOW != 0 {
        50.0
    } else if bits & GOLD != 0 {
        20.0
    } else {
        1.0
    };
    let env: f64 = MUTATIONS
        .iter()
        .filter(|(b, _, _)| *b > RAINBOW && bits & b != 0)
        .map(|(_, _, m)| m - 1.0)
        .sum();
    (c.value as f64 * w * w * variant * (1.0 + env))
        .round()
        .max(1.0) as u64
}

/// The colour a fruit shows: its crop's, or its strongest mutation's.
pub fn fruit_color(kind: u8, bits: u8) -> [f32; 3] {
    if bits & RAINBOW != 0 {
        [0.85, 0.35, 0.95]
    } else if bits & GOLD != 0 {
        [1.0, 0.8, 0.15]
    } else if bits & SHOCKED != 0 {
        [0.95, 0.95, 0.4]
    } else if bits & FROZEN != 0 {
        [0.75, 0.9, 1.0]
    } else if bits & (WET | CHILLED) != 0 {
        let c = crop(kind).fruit;
        [c[0] * 0.6 + 0.1, c[1] * 0.6 + 0.2, c[2] * 0.6 + 0.4]
    } else {
        crop(kind).fruit
    }
}

/// "1.2K", "3.4M": the HUD's compact sheckles. Contract's
/// `formatNumber(n, "compact")` exists, but a world-side string keeps the
/// shape a list of plain strings.
pub fn compact(n: u64) -> String {
    const UNITS: &[(u64, &str)] = &[
        (1_000_000_000_000, "T"),
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "K"),
    ];
    for &(unit, suffix) in UNITS {
        if n >= unit {
            let tenths = n * 10 / unit;
            return if tenths >= 1000 || tenths % 10 == 0 {
                format!("{}{suffix}", tenths / 10)
            } else {
                format!("{}.{}{suffix}", tenths / 10, tenths % 10)
            };
        }
    }
    n.to_string()
}

/// "4:05", "1:02:03".
pub fn clock(ms: u64) -> String {
    let s = ms.div_ceil(1000);
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_weight_and_mutations() {
        let carrot = kind_of("carrot").unwrap();
        assert_eq!(fruit_value(carrot, 0.25, 0), 18);
        assert_eq!(fruit_value(carrot, 0.5, 0), 72);
        assert_eq!(fruit_value(carrot, 0.25, GOLD), 360);
        assert_eq!(fruit_value(carrot, 0.25, GOLD | WET), 720);
        assert_eq!(fruit_value(carrot, 0.25, RAINBOW | WET | CHILLED), 2700);
        assert_eq!(mutation_names(GOLD | WET), "Gold Wet");
    }

    #[test]
    fn formats() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(1_000), "1K");
        assert_eq!(compact(1_250), "1.2K");
        assert_eq!(compact(125_000), "125K");
        assert_eq!(compact(3_400_000), "3.4M");
        assert_eq!(clock(245_000), "4:05");
        assert_eq!(clock(244_001), "4:05");
        assert_eq!(clock(3_723_000), "1:02:03");
    }

    #[test]
    fn catalogue_is_sorted_and_named() {
        assert!(CROPS.windows(2).all(|w| w[0].price <= w[1].price));
        assert!(CROPS
            .iter()
            .all(|c| c.slots >= 1 && (c.regrows == (c.fruit_s > 0))));
    }
}
