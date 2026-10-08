//! `feed.json` as typed rows (SPEC "Data"): every kind's payload fields, absent ones defaulted.

use std::collections::HashMap;

#[derive(serde::Deserialize)]
pub struct Doc {
    pub fonts: Fonts,
    pub messages: Vec<Msg>,
    pub rows: Vec<Raw>,
}

#[derive(serde::Deserialize)]
pub struct Msg {
    pub name: String,
    pub avatar: String,
    pub text: String,
}

pub type Fonts = HashMap<String, FontSpec>;

#[derive(serde::Deserialize)]
pub struct FontSpec {
    pub family: String,
    pub size: f64,
}

#[derive(serde::Deserialize, Default)]
pub struct Photo {
    pub src: String,
    pub w: f64,
    pub h: f64,
}
#[derive(serde::Deserialize, Default)]
pub struct Stroke {
    pub p: Vec<f64>,
    pub color: String,
    pub width: f64,
}
#[derive(serde::Deserialize, Default)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub color: String,
}
#[derive(serde::Deserialize, Default)]
pub struct Tok {
    pub t: String,
    pub k: String,
}
#[derive(serde::Deserialize, Default)]
pub struct Block {
    pub text: String,
    pub dir: String,
}
#[derive(serde::Deserialize, Default)]
pub struct Card {
    pub image: String,
    pub title: String,
    pub meta: String,
}

#[derive(serde::Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Raw {
    pub id: String,
    pub index: usize,
    pub kind: String,
    pub author: String,
    pub handle: String,
    pub avatar: String,
    pub minutes_ago: f64,
    pub caption: String,
    pub photo: Option<Photo>,
    pub title: String,
    pub thumbs: Vec<String>,
    pub duo_a: String,
    pub duo_b: String,
    pub image: String,
    pub bg: String,
    pub band: Vec<String>,
    pub strokes: Vec<Stroke>,
    pub dots: Vec<Dot>,
    pub icons: Vec<String>,
    pub chart: String,
    pub art: String,
    pub video: String,
    pub place: String,
    pub address: String,
    pub lat: f64,
    pub lon: f64,
    pub md: String,
    pub file: String,
    pub lang: String,
    pub lines: Vec<Vec<Tok>>,
    pub blocks: Vec<Block>,
    pub quote: String,
    pub by: String,
    pub font: String,
    pub cards: Vec<Card>,
    pub gif: String,
    pub webp: String,
    pub lottie: String,
    pub subtitle: String,
    pub rating: String,
    pub ends_in_sec: f64,
    pub rings: Vec<f64>,
    pub wave: Vec<f64>,
    pub updated_sec: f64,
    pub text: String,
    pub hue: i64,
    pub bars: Vec<i64>,
    pub count: f64,
    pub img0: f64,
    pub img_step: f64,
    pub num0: f64,
    pub m0: f64,
    pub m_step: f64,
    pub clock0: f64,
}
