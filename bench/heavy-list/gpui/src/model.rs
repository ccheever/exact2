use serde::Deserialize;

#[derive(Deserialize)]
pub struct Messages {
    pub messages: Vec<Message>,
}

#[derive(Deserialize)]
pub struct Message {
    pub id: String,
    pub author: String,
    pub avatar: String,
    #[serde(rename = "minutesAgo")]
    pub minutes: usize,
    pub paragraphs: Vec<Vec<Run>>,
    pub photos: Option<Vec<Photo>>,
    pub link: Option<Link>,
    pub quote: Option<Quote>,
    pub reactions: Option<Vec<Reaction>>,
}

#[derive(Deserialize)]
pub struct Run {
    pub t: String,
    pub s: Option<String>,
}

#[derive(Deserialize)]
pub struct Photo {
    pub src: String,
    pub w: f32,
    pub h: f32,
}

#[derive(Deserialize)]
pub struct Link {
    pub thumb: String,
    pub title: String,
    pub description: String,
    pub site: String,
}

#[derive(Deserialize)]
pub struct Quote {
    pub author: String,
    pub excerpt: String,
}

#[derive(Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: usize,
}

pub fn ago(minutes: usize) -> String {
    if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 1440 {
        format!("{}h ago", minutes / 60)
    } else {
        format!("{}d ago", minutes / 1440)
    }
}
