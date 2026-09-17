//! Pure gallery fixtures and logical interactions. No clock, I/O or pixel geometry.
//! A manual page bounds rendered records; it is not viewport virtualization.

pub const COUNTS: [usize; 3] = [100, 1_000, 25_000];
pub const PAGE_SIZE: usize = 12;
pub const MAX_ITEMS: usize = 25_000;
pub const MAX_SERIAL: u32 = 99_999;
pub const ASSET_COUNT: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Id(pub u32);

impl Id {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let digits = text
            .strip_prefix("photo-")
            .ok_or("invalid photo identity")?;
        if digits.len() != 5 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err("invalid photo identity");
        }
        let n = digits
            .parse::<u32>()
            .map_err(|_| "invalid photo identity")?;
        Ok(Self(n))
    }

    pub fn key(self) -> String {
        format!("photo-{:05}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Photos,
    Reorder,
    Sheet,
}

impl Mode {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        match text {
            "photos" => Ok(Self::Photos),
            "reorder" => Ok(Self::Reorder),
            "sheet" => Ok(Self::Sheet),
            _ => Err("unknown gallery mode"),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Photos => "photos",
            Self::Reorder => "reorder",
            Self::Sheet => "sheet",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Photo {
    pub id: String,
    pub title: &'static str,
    pub subtitle: String,
    pub asset: &'static str,
    pub alt: &'static str,
    pub tone: &'static str,
    pub notes: &'static str,
    pub rank: usize,
}

const ASSETS: [(&str, &str, &str, &str, &str); ASSET_COUNT] = [
    (
        "North shore",
        "Coastal light",
        "assets/north-shore.png",
        "Turquoise waves around a granite headland and golden coastal grass.",
        "#d3e3df",
    ),
    (
        "Ochre dunes",
        "Desert studies",
        "assets/ochre-dunes.png",
        "A curved ochre dune ridge with long blue shadows under an apricot sky.",
        "#dfc09f",
    ),
    (
        "Alpine water",
        "Still mornings",
        "assets/alpine-water.png",
        "Pines and granite mountains reflected in a quiet blue-green alpine lake.",
        "#c5d4d0",
    ),
    (
        "Winter ridge",
        "Above the clouds",
        "assets/winter-ridge.png",
        "A snowy mountain ridge catching warm light above lavender clouds.",
        "#d8dce8",
    ),
    (
        "Glasshouse",
        "Botanical notes",
        "assets/glasshouse.png",
        "Green monstera and fern leaves in a sunlit glasshouse with pale planters.",
        "#cdd8bf",
    ),
    (
        "Last light",
        "Evening elsewhere",
        "assets/last-light.png",
        "Terracotta walls and a cypress framing calm blue water at sunset.",
        "#e0c4ae",
    ),
];

pub fn photo(id: Id, rank: usize) -> Photo {
    let (title, category, asset, alt, tone) = ASSETS[id.0 as usize % ASSET_COUNT];
    Photo {
        id: id.key(), title, subtitle: format!("{category} · Study {:05}", id.0 + 1),
        asset, alt, tone, rank,
        notes: match id.0 % 3 {
            0 => "A study in space, texture and quiet light. Keep the horizon in view, then look for the smallest change in color.",
            1 => "Warm edges, cool shadows. An imagined place to revisit.",
            _ => "Collected for its changing layers: the foreground gives way to a softer distance. Follow the quiet reflections toward the horizon; there is always another detail to discover when you look again.",
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub token: u32,
    pub item: Id,
    /// None means the end; an identity means immediately before that record.
    pub before: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Return {
    None,
    Item(Id),
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gallery {
    order: Vec<Id>,
    pub requested: usize,
    pub mode: Mode,
    pub page: usize,
    pub selected: Option<Id>,
    pub viewer: bool,
    pub viewer_token: u32,
    pub moving: Option<Move>,
    pub returned: Return,
    pub revision: u32,
    pub notice: String,
    epoch: u32,
    next_serial: u32,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            order: (0..100).map(Id).collect(),
            requested: 100,
            mode: Mode::Photos,
            page: 0,
            selected: Some(Id(0)),
            viewer: false,
            viewer_token: 0,
            moving: None,
            returned: Return::None,
            revision: 0,
            notice: "Six imagined places. A collection to make your own.".into(),
            epoch: 0,
            next_serial: 100,
        }
    }
}

impl Gallery {
    pub fn ids(&self) -> &[Id] {
        &self.order
    }
    pub fn pages(&self) -> usize {
        self.order.len().div_ceil(PAGE_SIZE).max(1)
    }
    pub fn position(&self, id: Id) -> Option<usize> {
        self.order.iter().position(|n| *n == id)
    }
    pub fn rows(&self) -> Vec<Photo> {
        let first = self.page.min(self.pages() - 1) * PAGE_SIZE;
        self.order
            .iter()
            .enumerate()
            .skip(first)
            .take(PAGE_SIZE)
            .map(|(i, id)| photo(*id, i + 1))
            .collect()
    }
    pub fn selected_photo(&self) -> Option<Photo> {
        self.selected
            .and_then(|id| self.position(id).map(|i| photo(id, i + 1)))
    }
    fn token(&mut self) -> Result<u32, &'static str> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("interaction token exhausted; reopen this app")?;
        Ok(self.epoch)
    }
    fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    /// Loading a count deliberately resets this fixture. Identities always
    /// describe the same immutable photo study; interaction tokens never reset.
    pub fn load(&mut self, count: usize) -> Result<(), &'static str> {
        if !COUNTS.contains(&count) {
            return Err("choose 100, 1000 or 25000 records");
        }
        let token = self.token()?;
        let revision = self.revision.saturating_add(1);
        let mode = self.mode;
        *self = Self {
            order: (0..count as u32).map(Id).collect(),
            requested: count,
            next_serial: count as u32,
            epoch: token,
            viewer_token: token,
            mode,
            revision,
            notice: format!("Loaded {count} records. Order and deletions reset."),
            ..Self::default()
        };
        Ok(())
    }

    pub fn mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.viewer = false;
        self.moving = None;
        self.returned = Return::None;
        self.notice = match mode {
            Mode::Photos => "Open a study. Use Previous, Next and Back to return.",
            Mode::Reorder => "Pick up a card, preview its destination, then place or cancel.",
            Mode::Sheet => "Choose a sheet position, then scroll inside the reading panel.",
        }
        .into();
    }

    pub fn page_to(&mut self, page: usize) {
        self.page = page.min(self.pages() - 1);
    }

    pub fn select(&mut self, id: Id) -> Result<(), &'static str> {
        if self.position(id).is_none() {
            return Err("this record no longer exists");
        }
        self.selected = Some(id);
        Ok(())
    }

    pub fn open(&mut self, id: Id) -> Result<(), &'static str> {
        if self.position(id).is_none() {
            return Err("this record no longer exists");
        }
        let token = self.token()?;
        self.selected = Some(id);
        self.viewer = true;
        self.viewer_token = token;
        self.moving = None;
        self.returned = Return::None;
        self.notice =
            "Use Previous or Next to explore. Back returns to this study in your collection."
                .into();
        Ok(())
    }

    pub fn adjacent(&mut self, token: u32, forward: bool) -> Result<(), &'static str> {
        if !self.viewer || token != self.viewer_token {
            return Ok(());
        }
        let Some(pos) = self.selected.and_then(|id| self.position(id)) else {
            return Ok(());
        };
        let next = if forward {
            (pos + 1).min(self.order.len() - 1)
        } else {
            pos.saturating_sub(1)
        };
        self.open(self.order[next])
    }

    /// Resolve a return by the *current* order, including an off-page or moved
    /// source. Hosts still need to resolve its actual presentation rectangle.
    pub fn close(&mut self, token: u32) {
        if !self.viewer || token != self.viewer_token {
            return;
        }
        self.viewer = false;
        self.returned = match self
            .selected
            .and_then(|id| self.position(id).map(|i| (id, i)))
        {
            Some((id, i)) => {
                self.page = i / PAGE_SIZE;
                Return::Item(id)
            }
            None => Return::Removed,
        };
        self.notice = "Returned to the current location of this study.".into();
    }

    pub fn lift(&mut self, id: Id) -> Result<u32, &'static str> {
        let pos = self.position(id).ok_or("this record no longer exists")?;
        let token = self.token()?;
        self.moving = Some(Move {
            token,
            item: id,
            before: self.order.get(pos + 1).copied(),
        });
        self.selected = Some(id);
        self.notice = "Move preview only. The collection order is unchanged until Place.".into();
        Ok(token)
    }

    pub fn before(&mut self, token: u32, target: Option<Id>) -> Result<(), &'static str> {
        let Some(moving) = self.moving.filter(|m| m.token == token) else {
            return Ok(());
        };
        if target == Some(moving.item) {
            return Err("a card cannot be placed before itself");
        }
        if target.is_some_and(|id| self.position(id).is_none()) {
            return Err("destination no longer exists");
        }
        self.moving.as_mut().unwrap().before = target;
        Ok(())
    }

    pub fn nudge(&mut self, token: u32, forward: bool) -> Result<(), &'static str> {
        let Some(moving) = self.moving.filter(|m| m.token == token) else {
            return Ok(());
        };
        let others: Vec<_> = self
            .order
            .iter()
            .copied()
            .filter(|id| *id != moving.item)
            .collect();
        let position = moving
            .before
            .and_then(|id| others.iter().position(|n| *n == id))
            .unwrap_or(others.len());
        let next = if forward {
            (position + 1).min(others.len())
        } else {
            position.saturating_sub(1)
        };
        self.before(token, others.get(next).copied())
    }

    pub fn cancel(&mut self, token: u32) {
        if let Some(moving) = self.moving.filter(|m| m.token == token) {
            // A keyboard preview can visit another page. Remount the source
            // before the post-commit focus command returns to its control.
            if let Some(position) = self.position(moving.item) {
                self.page = position / PAGE_SIZE;
            }
            self.moving = None;
            self.notice = "Move cancelled. The collection order is unchanged.".into();
        }
    }

    /// Consume one current interaction. Stale or duplicate callbacks are no-ops.
    pub fn commit(&mut self, token: u32) {
        let Some(moving) = self.moving.filter(|m| m.token == token) else {
            return;
        };
        self.moving = None;
        let Some(source) = self.position(moving.item) else {
            return;
        };
        if moving.before.is_some_and(|id| self.position(id).is_none()) {
            self.notice = "Destination removed. Move cancelled.".into();
            return;
        }
        self.order.remove(source);
        let target = moving
            .before
            .and_then(|id| self.position(id))
            .unwrap_or(self.order.len());
        self.order.insert(target, moving.item);
        self.page = target / PAGE_SIZE;
        self.changed();
        self.notice = format!("Placed {} at position {}.", moving.item.key(), target + 1);
    }

    pub fn remove(&mut self, id: Id) -> Result<(), &'static str> {
        let index = self.position(id).ok_or("this record no longer exists")?;
        self.order.remove(index);
        if self
            .moving
            .is_some_and(|m| m.item == id || m.before == Some(id))
        {
            self.moving = None;
        }
        if self.selected == Some(id) {
            self.selected = self
                .order
                .get(index.min(self.order.len().saturating_sub(1)))
                .copied();
            if self.viewer {
                self.viewer = false;
                self.returned = Return::Removed;
            }
        }
        self.page = self.page.min(self.pages() - 1);
        self.changed();
        self.notice = format!("Removed {}. Any move involving it is cancelled.", id.key());
        Ok(())
    }

    /// Both live-count and never-reused serial bounds govern insertion.
    pub fn can_insert(&self) -> bool {
        self.order.len() < MAX_ITEMS && self.next_serial <= MAX_SERIAL
    }

    /// A distinct insertion for future concurrent-mutation interaction drives.
    pub fn insert_first(&mut self) -> Result<(), &'static str> {
        if !self.can_insert() {
            return Err("record bound reached; delete a record or reset");
        }
        let id = Id(self.next_serial);
        self.next_serial += 1;
        self.order.insert(0, id);
        if self.selected.is_none() {
            self.selected = Some(id);
        }
        self.changed();
        self.notice = format!("Inserted {}. Existing identities are unchanged.", id.key());
        Ok(())
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
